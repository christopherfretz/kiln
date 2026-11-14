//! `--pch-diff replay|snapshot` (WP-1036, `spec/harness.md` "PCH
//! differential"): kiln compiles a copy of each test without a PCH, and
//! again with its leading `#include`s emptied and `-include-pch` of a
//! PCH of them, and the two must agree: the
//! exit status, the diagnostics ([`pch_diff::same_diags`]), the LLVM IR
//! (`-S -emit-llvm`, [`ir_normalize`]d) and, for a `run` test, the
//! binary's exit status and output. Nothing is compared with the oracle,
//! and a test's own expectations are not checked: the variant asks only
//! whether the PCH changes the result.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use super::{Expect, Lang, Output, PchConsumer, Problem, Status, Test, TestCase, TestSpec};
use super::{indent, ir_normalize, line_diff};
use crate::util::pch_diff::{self, PchDiff};

/// The kinds the variant runs: those whose result is a compile (and a
/// run). A preprocessor, AST or CFG dump of a PCH consumer does not show
/// the header, by design.
fn compared(expect: Expect) -> bool {
    matches!(
        expect,
        Expect::CompileOk | Expect::CompileFail | Expect::Run | Expect::Ir | Expect::NoCrash
    )
}

/// Why `--pch-diff` leaves a test out, or `None` when it runs it.
pub(super) fn pch_diff_skip(
    spec: &TestSpec,
    source: &str,
    case: &TestCase,
    synthetic: bool,
) -> Option<String> {
    const MODE: &str = "pch-diff";
    if case.lang == Lang::C || case.is_header {
        return Some(format!("{MODE}: not a C++ source file"));
    }
    if !compared(spec.expect) {
        return Some(format!("{MODE}: `expect: {}` is not compared", spec.expect.as_str()));
    }
    if spec.pch.is_some() || !spec.units.is_empty() || !spec.header_units.is_empty() {
        return Some(format!("{MODE}: has `pch:`, `unit:` or `header-unit:` lines"));
    }
    if spec.args.iter().any(|a| a.starts_with("-include") || a == "-E" || a == "-x" || a == "-M") {
        return Some(format!("{MODE}: `args:` select an -include, -E, -M or -x"));
    }
    if pch_diff::leading_includes(source).is_empty() {
        if synthetic && !pch_diff::has_include(source) {
            return None;
        }
        return Some(format!("{MODE}: does not open with an #include"));
    }
    None
}

/// Under `--pch-diff-synthetic`, the PCHs of [`pch_diff::SYNTHETIC_HEADER`]
/// built so far, by producer command line: one per distinct set of
/// flags, shared by the run's tests (spec/harness.md "PCH differential").
#[derive(Debug, Default)]
pub struct SyntheticPchs {
    built: Mutex<HashMap<String, Arc<OnceLock<SyntheticPch>>>>,
}

/// Under `--pch-diff snapshot`, how many of the run's PCH builds took
/// each producer shape ([`pch_diff::producer_shape`]: the single pass,
/// or two passes and why), printed after the accounting (WP-1404). A
/// shared synthetic PCH counts once.
#[derive(Debug, Default)]
pub struct ProducerShapes {
    shapes: Mutex<std::collections::BTreeMap<String, usize>>,
}

impl ProducerShapes {
    /// Counts the producer whose standard error is `stderr`.
    fn add(&self, stderr: &str) {
        let shape = pch_diff::producer_shape(stderr).unwrap_or_else(|| {
            "no snapshot pass (the producer wrote no snapshot-timing line)".to_owned()
        });
        let mut m = self.shapes.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *m.entry(shape).or_default() += 1;
    }

    /// `pch-diff snapshot producers: N built: S single pass, T two
    /// passes`, then each shape's count; empty when none was counted.
    pub fn format(&self) -> String {
        use std::fmt::Write as _;
        let m = self.shapes.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if m.is_empty() {
            return String::new();
        }
        let total: usize = m.values().sum();
        let single = m.get("single pass").copied().unwrap_or(0);
        let mut s = format!(
            "pch-diff snapshot producers: {total} built: {single} single pass, {} two passes\n",
            total - single
        );
        for (shape, n) in m.iter() {
            let _ = writeln!(s, "  {n:>5}  {shape}");
        }
        s
    }
}

/// A synthetic PCH and its producer's stderr, or why it does not build.
type SyntheticPch = Result<(PathBuf, String), String>;

/// Whether two runs of the test binary agree: exit status, stdout and
/// stderr, with the binary's own name made one
/// ([`pch_diff::run_output_normalize`]).
fn same_run(a: &Output, b: &Output) -> bool {
    let norm = |o: &[u8]| pch_diff::run_output_normalize(&String::from_utf8_lossy(o));
    a.code() == b.code() && norm(&a.stdout) == norm(&b.stdout) && norm(&a.stderr) == norm(&b.stderr)
}

/// One build of the test: the IR compile and, for a `run` test, the
/// binary's run.
struct Build {
    compile: Output,
    ir: Option<String>,
    run: Option<Output>,
}

impl Test<'_> {
    /// The test's directory, relative to the repository (what its quoted
    /// `#include`s are found in; the variant compiles a copy).
    pub(super) fn test_dir(&self) -> PathBuf {
        match Path::new(&self.case.name).parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        }
    }

    /// The variant's verdict for `spec` (`source`: its text).
    pub(super) fn pch_diff(
        &mut self,
        spec: &TestSpec,
        source: &str,
        mode: PchDiff,
    ) -> (Status, Option<String>, String) {
        self.pch_path = None;
        let r = self.pch_diff_check(spec, source, mode);
        let path = self.pch_path.take();
        match r {
            Ok(Ok(path)) => (Status::Pass, Some(path), String::new()),
            Ok(Err(why)) => (Status::Skip, Some(format!("pch-diff: {why}")), String::new()),
            // The path the consumer took, when it got that far: the
            // accounting counts failures by path too.
            Err(Problem::Fail(d)) => (Status::Fail, path, d),
            Err(Problem::Broken(d)) => (Status::Error, path, d),
            Err(Problem::Killed { mib, details }) => {
                (Status::Killed, Some(format!("memory limit {mib} MiB")), details)
            }
        }
    }

    /// `Ok(Ok(path))`: the two builds agree, and the PCH took `path`;
    /// `Ok(Err(why))` skips the test.
    fn pch_diff_check(
        &mut self,
        spec: &TestSpec,
        source: &str,
        mode: PchDiff,
    ) -> Result<Result<String, String>, Problem> {
        let dir = self.scratch()?.to_path_buf();
        let includes = pch_diff::leading_includes(source);
        let copy_dir = dir.join("src");
        let copy = copy_dir.join(Path::new(&self.case.name).file_name().unwrap_or_default());
        fs::create_dir_all(&copy_dir)
            .map_err(|e| Problem::Broken(format!("cannot create {}: {e}", copy_dir.display())))?;
        write_file(&copy, source)?;
        self.pch_src = Some(copy.clone());
        let plain = self.pch_diff_build(spec, "plain");
        self.pch_src = None;
        let plain = match plain {
            Err(Problem::Fail(d)) => return Ok(Err(format!("kiln fails without a PCH: {d}"))),
            r => r?,
        };
        let synthetic = includes.is_empty();
        let (plain, producer, pch) = if synthetic {
            match self.pch_diff_synthetic(spec, &dir, &copy, &plain)? {
                Ok(r) => r,
                Err(why) => return Ok(Err(why)),
            }
        } else {
            let header = dir.join("pchdiff.h");
            let pch = dir.join("pchdiff.pch");
            write_file(&header, &pch_diff::header_text(&includes))?;
            let producer = self.pch_diff_produce(spec, &header, &pch, true)?;
            if !producer.success() {
                if !plain.compile.success() {
                    return Ok(Err("the leading #includes do not compile".to_owned()));
                }
                return Err(Problem::Fail(format!(
                    "kiln compiles the test but not a PCH of its leading #includes ({})\n{}",
                    producer.describe_end(),
                    indent(&producer.stderr_str())
                )));
            }
            write_file(&copy, &pch_diff::without_includes(source, &includes))?;
            (plain, producer.stderr_str(), pch)
        };
        let mut path = mode.as_str().to_owned();
        self.pch_flags = vec!["-include-pch".into(), pch.into_os_string()];
        self.pch_flags.extend(mode.consumer_flags().iter().map(Into::into));
        // WP-1403: a consumer with one more (empty) `-I` directory than
        // its producer, so the snapshot's lookups are repeated.
        let extra =
            mode == PchDiff::Snapshot && self.cfg.pch_extra_dir && extra_dir_case(&self.case.name);
        if extra {
            let d = dir.join("consumer-only");
            fs::create_dir_all(&d)
                .map_err(|e| Problem::Broken(format!("cannot create {}: {e}", d.display())))?;
            self.pch_flags.extend(["-I".into(), d.into_os_string()]);
        }
        self.pch_src = Some(copy);
        self.pch_consumer =
            PchConsumer { snapshot_timing: mode == PchDiff::Snapshot, iquote: !synthetic };
        let with = self.pch_diff_build(spec, "pch");
        (self.pch_src, self.pch_flags) = (None, Vec::new());
        self.pch_consumer = PchConsumer::default();
        let with = match with {
            // As for the reference build: a test's memory cap is sized for
            // the test alone, and with the synthetic prefix both builds sit
            // at its edge (`contracts-many.cpp`: the restore needs about
            // 1296 MiB of address space, the `-include` build 1342 MiB,
            // the cap is 1280 MiB), so which one is killed varies.
            Err(Problem::Killed { mib, .. }) if synthetic => {
                return Ok(Err(format!(
                    "the synthetic prefix exceeds the test's memory limit ({mib} MiB)"
                )));
            }
            r => r?,
        };
        if mode == PchDiff::Snapshot {
            path = pch_diff::snapshot_path(&producer, &with.compile.stderr_str());
        }
        self.pch_path = Some(path.clone());
        let what = if synthetic {
            format!("with -include-pch of the synthetic prefix ({})", mode.consumer_label())
        } else {
            format!("with -include-pch of the leading #includes ({})", mode.consumer_label())
        };
        compare(&plain, &with, &producer, &what)?;
        if extra
            && path == pch_diff::RESTORED
            && !pch_diff::lookups_repeated(&with.compile.stderr_str())
        {
            return Err(Problem::Fail(
                "the consumer with an extra -I directory restored without repeating the snapshot's lookups (--pch-diff-extra-dir)"
                    .to_owned(),
            ));
        }
        if let Some(allow) = &self.cfg.pch_require_restore
            && path != pch_diff::RESTORED
            && allow.allows(&path).is_none()
        {
            return Err(Problem::Fail(format!(
                "the consumer replayed the header instead of restoring the snapshot ({path}), \
                 and {} allows no such reason (--pch-diff-require-restore)",
                pch_diff::ALLOW_LIST
            )));
        }
        Ok(Ok(path))
    }

    /// `--pch-diff-synthetic` (a test without `#include`s): the
    /// reference is the test with the synthetic header as an `-include`,
    /// provided that changes nothing kiln reports. `Ok(Ok((reference,
    /// producer stderr, pch)))`, or `Ok(Err(why))` to skip the test.
    fn pch_diff_synthetic(
        &mut self,
        spec: &TestSpec,
        dir: &Path,
        copy: &Path,
        plain: &Build,
    ) -> Result<Result<(Build, String, PathBuf), String>, Problem> {
        let header = dir.join("pchdiff.h");
        write_file(&header, pch_diff::SYNTHETIC_HEADER)?;
        self.pch_flags = vec!["-include".into(), header.into_os_string()];
        self.pch_src = Some(copy.to_path_buf());
        self.pch_consumer.iquote = false;
        let textual = self.pch_diff_build(spec, "textual");
        (self.pch_src, self.pch_flags) = (None, Vec::new());
        self.pch_consumer = PchConsumer::default();
        let textual = match textual {
            Err(Problem::Fail(_)) => {
                return Ok(Err("kiln fails with the synthetic prefix".to_owned()));
            }
            // The test's memory cap is sized for the test alone.
            Err(Problem::Killed { mib, .. }) => {
                return Ok(Err(format!(
                    "the synthetic prefix exceeds the test's memory limit ({mib} MiB)"
                )));
            }
            r => r?,
        };
        let (p, t) = (plain.compile.stderr_str(), textual.compile.stderr_str());
        // Where a stack guard fires depends on how deep the compiler's
        // own frames are when the main file starts, which a restore
        // changes by a few frames: the depth is not a property of the
        // prefix.
        if p.contains("stack exhausted") {
            return Ok(
                Err("the test measures the stack depth, which the prefix shifts".to_owned()),
            );
        }
        let same_run = match (&plain.run, &textual.run) {
            (Some(a), Some(b)) => same_run(a, b),
            (a, b) => a.is_none() == b.is_none(),
        };
        if plain.compile.success() != textual.compile.success()
            || pch_diff::diag_lines(&p) != pch_diff::diag_lines(&t)
            || !same_run
        {
            return Ok(Err("the synthetic prefix changes the test's result".to_owned()));
        }
        Ok(self.synthetic_pch(spec).map(|(pch, producer)| (textual, producer, pch)))
    }

    /// The PCH of `header`: the test's flags, the producer's
    /// ([`PchDiff::producer_flags`]), and the test's directory for its
    /// quoted `#include`s (the header is in the scratch directory). The
    /// test's own flags come in the consumer's order ([`Self::command`]):
    /// a snapshot's identity is the command line in order, so the
    /// producer also gets the `-fmodule-file=` flags of its `module:`
    /// lines (WP-1395).
    fn pch_diff_produce(
        &mut self,
        spec: &TestSpec,
        header: &Path,
        pch: &Path,
        iquote: bool,
    ) -> Result<Output, Problem> {
        let c = self.pch_diff_producer(spec, header, pch, iquote);
        let o = self.exec("the compiler", &c, self.cfg.compile_timeout)?;
        if o.code().is_none() || o.stderr_str().contains("panicked at") {
            return Err(Problem::Fail(format!(
                "kiln crashed building the PCH ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        if self.cfg.pch_diff == Some(PchDiff::Snapshot) && o.success() {
            self.cfg.pch_producers.add(&o.stderr_str());
        }
        Ok(o)
    }

    /// The command of [`Self::pch_diff_produce`] (`iquote`: with the
    /// test's directory as `-iquote`).
    fn pch_diff_producer(
        &self,
        spec: &TestSpec,
        header: &Path,
        pch: &Path,
        iquote: bool,
    ) -> Command {
        let program = self.cfg.compiler.clone();
        let mut c = Command::new(&program);
        crate::util::isolate_caches(&mut c);
        c.current_dir(&self.cfg.root);
        c.env("KILN_LLVM_DIR", &self.cfg.llvm_dir);
        c.arg(format!("-std={}", spec.std));
        c.args(self.no_crash_reports(&program));
        if let Some(n) = self.clang_compat(&program, spec) {
            c.arg(format!("-fclang-compat={n}"));
        }
        if self.cfg.pch_diff == Some(PchDiff::Snapshot) {
            // The consumer's order (`Self::compile`): the snapshot's
            // identity is the command line, in order.
            c.args(self.meta_header_flags(true));
        }
        c.args(self.args_for(&program, spec));
        c.args(self.module_flags(&program, spec, spec.modules.len()));
        if iquote {
            c.arg("-iquote").arg(self.test_dir());
        }
        let mode = self.cfg.pch_diff.unwrap_or(PchDiff::Replay);
        c.args(mode.producer_flags());
        if mode == PchDiff::Snapshot {
            // The reason a producer writes no snapshot, and its shape
            // ([`pch_diff::producer_shape`]).
            c.env(pch_diff::SNAPSHOT_TIMING_ENV, "1");
        }
        c.args(["-x", "c++-header"]).arg(header).arg("-o").arg(pch);
        c
    }

    /// The PCH of [`pch_diff::SYNTHETIC_HEADER`] for `spec`'s flags and
    /// the producer's stderr, built once per distinct producer command
    /// line in the run ([`SyntheticPchs`]); `Err(why)` when it does not
    /// build (the test is skipped).
    fn synthetic_pch(&mut self, spec: &TestSpec) -> SyntheticPch {
        let cfg = self.cfg;
        let Some(cache) = &cfg.pch_synthetic else {
            return Err("no synthetic prefix in this run".to_owned());
        };
        // The key: the command line without the header and the output.
        let probe = self.pch_diff_producer(spec, Path::new("H"), Path::new("O"), false);
        let key = format!("{:?}", probe.get_args().collect::<Vec<_>>());
        let slot = {
            let mut m = cache.built.lock().expect("no worker panics while holding the lock");
            let n = m.len();
            let slot = Arc::clone(m.entry(key).or_default());
            (slot, n)
        };
        let (slot, n) = slot;
        let built = slot.get_or_init(|| {
            let dir = self.cfg.scratch.join("synthetic-pch").join(n.to_string());
            let (header, pch) = (dir.join("pchdiff.h"), dir.join("pchdiff.pch"));
            let made = fs::create_dir_all(&dir)
                .and_then(|()| fs::write(&header, pch_diff::SYNTHETIC_HEADER));
            if let Err(e) = made {
                return Err(format!("cannot write {}: {e}", header.display()));
            }
            match self.pch_diff_produce(spec, &header, &pch, false) {
                Ok(o) if o.success() => Ok((pch, o.stderr_str())),
                Ok(o) => Err(format!(
                    "the synthetic prefix does not build a PCH with the test's flags ({})",
                    o.describe_end()
                )),
                Err(Problem::Fail(d) | Problem::Broken(d)) => Err(format!(
                    "the synthetic prefix's PCH failed: {}",
                    d.lines().next().unwrap_or("")
                )),
                Err(Problem::Killed { mib, .. }) => {
                    Err(format!("the synthetic prefix's PCH hit the memory limit ({mib} MiB)"))
                }
            }
        });
        built.clone()
    }

    /// One build (`tag` names its files): for a `run` test a binary (in
    /// one step or [two](Self::two_step), which writes the module files)
    /// and its run, then `-S -emit-llvm`. A crash is [`Problem::Fail`].
    fn pch_diff_build(&mut self, spec: &TestSpec, tag: &str) -> Result<Build, Problem> {
        let dir = self.scratch()?.to_path_buf();
        let mut run = None;
        if spec.expect == Expect::Run {
            let exe = dir.join(format!("{tag}.out"));
            let o = self.compile_run_test(spec, &exe)?;
            if o.success() {
                let stdin = self.stdin_bytes(spec)?;
                run = Some(self.execute_binary(&exe, spec, stdin.as_deref(), false)?);
            }
        }
        let ll = dir.join(format!("{tag}.ll"));
        let compile = self.compile(spec, &["-S", "-emit-llvm"], Some(&ll))?;
        let ir = if compile.success() { fs::read_to_string(&ll).ok() } else { None };
        Ok(Build { compile, ir, run })
    }
}

/// The PCH differential's verdict on the two builds: the same success,
/// the same diagnostics (the replay's first ones being the header's,
/// [`pch_diff::same_diags`]), the same LLVM IR and the same run.
fn compare(plain: &Build, with: &Build, producer: &str, what: &str) -> Result<(), Problem> {
    let (p, w) = (plain.compile.stderr_str(), with.compile.stderr_str());
    if plain.compile.success() != with.compile.success() {
        return Err(Problem::Fail(format!(
            "kiln {} without a PCH but {} {what}\n--- without ---\n{}--- with ---\n{}",
            if plain.compile.success() { "succeeds" } else { "fails" },
            if with.compile.success() { "succeeds" } else { "fails" },
            indent(&p),
            indent(&w)
        )));
    }
    if !pch_diff::same_diags(&p, producer, &w) {
        return Err(Problem::Fail(format!(
            "the diagnostics differ {what}\n--- diff (-without +with) ---\n{}",
            indent(&line_diff(&p, &w))
        )));
    }
    if let (Some(a), Some(b)) = (&plain.ir, &with.ir) {
        let (a, b) = (
            pch_diff::ir_pch_normalize(&ir_normalize(a)),
            pch_diff::ir_pch_normalize(&ir_normalize(b)),
        );
        if a != b {
            return Err(Problem::Fail(format!(
                "the LLVM IR differs {what}\n--- diff (-without +with) ---\n{}",
                indent(&line_diff(&a, &b))
            )));
        }
    }
    if let (Some(a), Some(b)) = (&plain.run, &with.run)
        && !same_run(a, b)
    {
        return Err(Problem::Fail(format!(
            "the test binary's run differs {what}: {} vs {}\n--- stdout without ---\n{}--- stdout with ---\n{}",
            a.describe_end(),
            b.describe_end(),
            indent(&a.stdout_str()),
            indent(&b.stdout_str())
        )));
    }
    Ok(())
}

/// Whether `--pch-diff-extra-dir` gives the test `name` the extra
/// directory: every other test, by its name, so the slice runs both the
/// tolerant path and the fast path.
fn extra_dir_case(name: &str) -> bool {
    name.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b))) % 2 == 0
}

/// Writes `text` to `path`; a failure is [`Problem::Broken`].
fn write_file(path: &Path, text: &str) -> Result<(), Problem> {
    fs::write(path, text)
        .map_err(|e| Problem::Broken(format!("cannot write {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert!(compared(Expect::Run) && compared(Expect::Ir));
        assert!(!compared(Expect::Pp) && !compared(Expect::Ast));
    }
}
