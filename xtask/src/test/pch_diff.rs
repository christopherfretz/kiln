//! `--pch-diff load|replay|check` (WP-1036, `spec/harness.md` "PCH
//! differential"): kiln compiles a copy of each test without a PCH, and
//! again with its leading `#include`s emptied and `-include-pch` of a
//! PCH of them, and the two must agree: the
//! exit status, the diagnostics ([`pch_diff::same_diags`]), the LLVM IR
//! (`-S -emit-llvm`, [`ir_normalize`]d) and, for a `run` test, the
//! binary's exit status and output. Nothing is compared with the oracle,
//! and a test's own expectations are not checked: the variant asks only
//! whether the PCH changes the result.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Expect, Lang, Output, Problem, Status, Test, TestCase, TestSpec};
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
pub(super) fn pch_diff_skip(spec: &TestSpec, source: &str, case: &TestCase) -> Option<String> {
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
        return Some(format!("{MODE}: does not open with an #include"));
    }
    None
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
        match self.pch_diff_check(spec, source, mode) {
            Ok(Ok(path)) => (Status::Pass, Some(path), String::new()),
            Ok(Err(why)) => (Status::Skip, Some(format!("pch-diff: {why}")), String::new()),
            Err(Problem::Fail(d)) => (Status::Fail, None, d),
            Err(Problem::Broken(d)) => (Status::Error, None, d),
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
        let write = |path: &Path, text: &str| {
            fs::write(path, text)
                .map_err(|e| Problem::Broken(format!("cannot write {}: {e}", path.display())))
        };
        fs::create_dir_all(&copy_dir)
            .map_err(|e| Problem::Broken(format!("cannot create {}: {e}", copy_dir.display())))?;
        write(&copy, source)?;
        self.pch_src = Some(copy.clone());
        let plain = self.pch_diff_build(spec, "plain");
        self.pch_src = None;
        let plain = match plain {
            Err(Problem::Fail(d)) => return Ok(Err(format!("kiln fails without a PCH: {d}"))),
            r => r?,
        };
        let header = dir.join("pchdiff.h");
        let pch = dir.join("pchdiff.pch");
        write(&header, &pch_diff::header_text(&includes))?;
        let producer = self.pch_diff_produce(spec, &header, &pch)?;
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
        let mut path = match mode {
            PchDiff::Replay | PchDiff::Snapshot => mode.as_str().to_owned(),
            _ => pch_diff::path_note(&producer.stderr_str())
                .unwrap_or_else(|| "no declaration image".to_owned()),
        };
        write(&copy, &pch_diff::without_includes(source, &includes))?;
        self.pch_flags =
            vec!["-include-pch".into(), pch.into_os_string(), mode.consumer_flag().into()];
        self.pch_src = Some(copy);
        self.pch_snapshot_timing = mode == PchDiff::Snapshot;
        let with = self.pch_diff_build(spec, "pch");
        (self.pch_src, self.pch_flags) = (None, Vec::new());
        self.pch_snapshot_timing = false;
        let with = with?;
        if mode == PchDiff::Snapshot {
            path = pch_diff::snapshot_path(&producer.stderr_str(), &with.compile.stderr_str());
        }
        let what = format!("with -include-pch of the leading #includes ({})", mode.consumer_flag());
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
        if !pch_diff::same_diags(&p, &producer.stderr_str(), &w) {
            return Err(Problem::Fail(format!(
                "the diagnostics differ {what}\n--- diff (-without +with) ---\n{}",
                indent(&line_diff(&p, &w))
            )));
        }
        if let (Some(a), Some(b)) = (&plain.ir, &with.ir) {
            let (a, b) = (ir_normalize(a), ir_normalize(b));
            if a != b {
                return Err(Problem::Fail(format!(
                    "the LLVM IR differs {what}\n--- diff (-without +with) ---\n{}",
                    indent(&line_diff(&a, &b))
                )));
            }
        }
        if let (Some(a), Some(b)) = (&plain.run, &with.run) {
            let same = a.code() == b.code() && a.stdout == b.stdout && a.stderr == b.stderr;
            if !same {
                return Err(Problem::Fail(format!(
                    "the test binary's run differs {what}: {} vs {}\n--- stdout without ---\n{}--- stdout with ---\n{}",
                    a.describe_end(),
                    b.describe_end(),
                    indent(&a.stdout_str()),
                    indent(&b.stdout_str())
                )));
            }
        }
        Ok(Ok(path))
    }

    /// The PCH of `header`: the test's flags, the producer's
    /// ([`pch_diff::PRODUCER_FLAGS`]), and the test's directory for its
    /// quoted `#include`s (the header is in the scratch directory).
    fn pch_diff_produce(
        &mut self,
        spec: &TestSpec,
        header: &Path,
        pch: &Path,
    ) -> Result<Output, Problem> {
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
        c.arg("-iquote").arg(self.test_dir());
        c.args(pch_diff::PRODUCER_FLAGS);
        if self.cfg.pch_diff == Some(PchDiff::Snapshot) {
            c.arg(pch_diff::SNAPSHOT_PRODUCER_FLAG);
        }
        c.args(["-x", "c++-header"]).arg(header).arg("-o").arg(pch);
        let o = self.exec("the compiler", &c, self.cfg.compile_timeout)?;
        if o.code().is_none() || o.stderr_str().contains("panicked at") {
            return Err(Problem::Fail(format!(
                "kiln crashed building the PCH ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        Ok(o)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds() {
        assert!(compared(Expect::Run) && compared(Expect::Ir));
        assert!(!compared(Expect::Pp) && !compared(Expect::Ast));
    }
}
