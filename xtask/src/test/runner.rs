//! Executing one test (`spec/harness.md` "Test kinds").

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::compare::{line_diff, pp_diff};
use super::diag_parse::{self, Diagnostic};
use super::directives::{
    self, CompilerKind, Expect, Lang, PpCompare, Severity, StdinInput, StdoutExpect, TestSpec,
};
use super::discover::TestCase;
use super::inst_report;
use crate::clang_ast::{DumpOptions, normalize};
use crate::util::oven_report::{self, Report};
use crate::util::process::{self, MemLimit, Output, Shown};
use kiln_abi::dump_blocks::{record_layout_blocks, vtable_blocks};

#[path = "gcc.rs"]
mod gcc;

/// Everything a worker needs; shared read-only between threads.
#[derive(Debug)]
pub struct Config {
    /// Repository root; compilers run with this as their working directory.
    pub root: PathBuf,
    pub compiler: PathBuf,
    pub compiler_kind: CompilerKind,
    /// The compiler's clang version, when it is clang (`requires: clang>=`).
    pub compiler_clang: Option<directives::ClangVersion>,
    /// The oracle compiler; `None` disables oracle comparisons (`--no-oracle`).
    pub oracle: Option<PathBuf>,
    /// What the oracle is (its stack reservation is allowed for if kiln).
    pub oracle_kind: CompilerKind,
    /// The oracle's clang version, when it is clang (`requires: clang>=`).
    pub oracle_clang: Option<directives::ClangVersion>,
    /// The GCC oracle of `oracle: gcc` tests (`KILN_GCC`); `None` drops
    /// their GCC comparison (`spec/harness.md` "GCC oracle").
    pub gcc: Option<PathBuf>,
    /// The GCC oracle's installation prefix (`%{gcc_toolchain}`, what
    /// `--gcc-toolchain=` takes); `None` without a GCC oracle.
    pub gcc_toolchain: Option<PathBuf>,
    /// `--meta-header`: the directory of the compatibility headers that
    /// stand in for `tests/support/kiln_meta*.h` (they include the real
    /// `<meta>`); `None` outside the header-differential mode.
    pub meta_header: Option<PathBuf>,
    /// `/usr/lib/llvm-N` of the oracle release: kiln++ runs with it as
    /// `KILN_LLVM_DIR` (`spec/harness.md` "Oracle toolchain").
    pub llvm_dir: PathBuf,
    /// The LLVM major of `llvm_dir`, the backend that consumes kiln's IR
    /// (`requires: llvm>=`).
    pub llvm_major: u32,
    pub update: bool,
    /// Per-run scratch directory (under the cargo target dir).
    pub scratch: PathBuf,
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
    /// `--pp-compare`: overrides every test's `pp-compare:`.
    pub pp_compare: Option<PpCompare>,
    /// `--mem-limit`: the address-space cap of every process a test runs;
    /// `None` (`--mem-limit 0`) leaves only `mem<=N` budgets
    /// (`spec/harness.md` "Resource limits").
    pub mem_limit: Option<MemLimit>,
    /// `KILN_REQUIRE_DEBUGGERS=1`: a `debugger` test whose debugger is not
    /// on `PATH` is an ERROR rather than a SKIP (`spec/harness.md`
    /// "Debugger tests").
    pub require_debuggers: bool,
}

/// Outcome of one test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Status {
    Pass,
    Fail,
    XFail,
    XPass,
    Skip,
    /// The test itself is broken (bad directives, missing file, the oracle
    /// rejected it); never masked by `xfail`.
    Error,
    /// A process of the test exceeded its address-space cap (`--mem-limit`
    /// or `mem<=N`); never masked by `xfail`.
    Killed,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::XFail => "XFAIL",
            Self::XPass => "XPASS",
            Self::Skip => "SKIP",
            Self::Error => "ERROR",
            Self::Killed => "KILLED",
        }
    }

    /// Does this outcome make the run fail?
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Fail | Self::XPass | Self::Error | Self::Killed)
    }
}

#[derive(Clone, Debug)]
pub struct TestResult {
    pub name: String,
    pub status: Status,
    /// The skip/xfail reason, when one applies.
    pub reason: Option<String>,
    /// Why the test failed (also filled for xfail, for `--verbose`).
    pub details: String,
    /// Commands that were run, for `--verbose`.
    pub log: Vec<String>,
    /// `--update` rewrote this test's `.expected` file.
    pub updated: bool,
    /// `advisory: oracle crashed (...)`: the GCC oracle crashed, hung or
    /// gave up, so its comparison was dropped; the status is kiln's alone.
    pub advisory: Option<String>,
    /// The GCC oracle compiled the test and its verdict or output was
    /// compared with kiln's (not dropped as advisory); the summary counts
    /// these (`spec/harness.md` "Output").
    pub gcc_compared: bool,
    pub elapsed: Duration,
}

enum Problem {
    Fail(String),
    Broken(String),
    /// A process ran out of address space under the test's cap of `mib`.
    Killed {
        mib: u64,
        details: String,
    },
}

impl Problem {
    fn text(&self) -> &str {
        match self {
            Self::Fail(d) | Self::Broken(d) | Self::Killed { details: d, .. } => d,
        }
    }
}

type Check = Result<(), Problem>;

/// Runs one test. `index` names its scratch subdirectory.
pub fn run_test(cfg: &Config, case: &TestCase, index: usize) -> TestResult {
    let start = Instant::now();
    let mut t = Test {
        cfg,
        case,
        dir: cfg.scratch.join(format!("t{index}")),
        log: Vec::new(),
        updated: false,
        inst_report: None,
        mem_limit: cfg.mem_limit,
        advisory: None,
        gcc_compared: false,
    };
    let (status, reason, details) = t.evaluate();
    if t.dir.exists() {
        let _ = fs::remove_dir_all(&t.dir);
    }
    TestResult {
        name: case.name.clone(),
        status,
        reason,
        details,
        log: t.log,
        updated: t.updated,
        advisory: t.advisory,
        gcc_compared: t.gcc_compared,
        elapsed: start.elapsed(),
    }
}

struct Test<'a> {
    cfg: &'a Config,
    case: &'a TestCase,
    dir: PathBuf,
    log: Vec<String>,
    updated: bool,
    /// The last compile's masked instantiation report (`inst-report:`).
    inst_report: Option<String>,
    /// This test's cap: `--mem-limit` lowered by its `mem<=N`.
    mem_limit: Option<MemLimit>,
    /// Set when the GCC oracle's comparison was dropped ([`TestResult::advisory`]).
    advisory: Option<String>,
    /// Set when the GCC oracle gave a verdict ([`TestResult::gcc_compared`]).
    gcc_compared: bool,
}

impl Test<'_> {
    /// Why this run's configuration cannot run `spec`: a kiln-specific
    /// test under another compiler, a test that needs `%{gcc_toolchain}`
    /// without GCC, or one the header-differential mode leaves out.
    fn mode_skip(&self, spec: &TestSpec, source: &str) -> Option<String> {
        if spec.is_kiln_specific() && self.cfg.compiler_kind != CompilerKind::Kiln {
            let directive = if spec.oven.is_empty() { "inst-report:" } else { "oven:" };
            return Some(format!(
                "not supported by this compiler (kiln-specific `{directive}` directive)"
            ));
        }
        if self.cfg.gcc_toolchain.is_none()
            && spec.args.iter().any(|a| a.contains("%{gcc_toolchain}"))
        {
            let reason = "no GCC oracle, so no `%{gcc_toolchain}` (the test needs GCC's libstdc++)";
            return Some(reason.to_owned());
        }
        if self.cfg.meta_header.is_some() {
            return meta_header_skip(spec, source);
        }
        None
    }

    /// Builds a compile-kind test's `module:` units for the compiler under
    /// test (a `run` test builds them with its program); `Some` is the
    /// verdict when that fails.
    fn prebuild_modules(&mut self, spec: &TestSpec) -> Option<(Status, Option<String>, String)> {
        if spec.expect == Expect::Run {
            return None;
        }
        let p = self.build_modules(&self.cfg.compiler.clone(), spec, false).err()?;
        Some(match p {
            Problem::Killed { mib, details } => {
                (Status::Killed, Some(format!("memory limit {mib} MiB")), details)
            }
            Problem::Fail(d) | Problem::Broken(d) => (Status::Error, None, d),
        })
    }

    fn evaluate(&mut self) -> (Status, Option<String>, String) {
        let source = match fs::read(&self.case.path) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => return (Status::Error, None, format!("cannot read test: {e}")),
        };
        let spec = match directives::parse(&source, self.case.lang) {
            Ok(s) => s,
            Err(errs) => {
                let mut d = String::from("malformed directives:\n");
                for e in errs {
                    let _ = writeln!(d, "  {e}");
                }
                return (Status::Error, None, d);
            }
        };
        if let Some(n) = spec.mem_mib {
            let budget = MemLimit { mib: n };
            self.mem_limit = Some(self.mem_limit.map_or(budget, |l| l.min(budget)));
        }
        let kind = self.cfg.compiler_kind;
        if let Some(r) = spec.skip_reason(kind) {
            return (Status::Skip, Some(r.to_owned()), String::new());
        }
        // The compiler under test (and the oracle) target this host unless
        // the test pins a `--target`.
        if let Some(r) = spec.unmet_requirement(
            std::env::consts::ARCH,
            &self.clangs_run(&spec),
            &self.llvms_run(&spec),
        ) {
            return (Status::Skip, Some(r), String::new());
        }
        if let Some(reason) = self.mode_skip(&spec, &source) {
            return (Status::Skip, Some(reason), String::new());
        }
        if (!spec.modules.is_empty() || !spec.units.is_empty()) && !kind.speaks_clang() {
            let reason = "not supported by this compiler (`module:` needs kiln or clang)";
            return (Status::Skip, Some(reason.to_owned()), String::new());
        }
        if spec.verify && !kind.speaks_clang() {
            let reason = "not supported by this compiler (`verify` needs kiln or clang)";
            return (Status::Skip, Some(reason.to_owned()), String::new());
        }
        if matches!(spec.expect, Expect::Cfg | Expect::Ast) && !kind.speaks_clang() {
            let name = if spec.expect == Expect::Cfg { "cfg" } else { "ast" };
            let reason =
                format!("not supported by this compiler (`expect: {name}` needs kiln or clang)");
            return (Status::Skip, Some(reason), String::new());
        }
        if let Some((status, why)) = debugger_gate(&spec, self.cfg.require_debuggers) {
            return (status, Some(why), String::new());
        }
        if let Some(dump) = DumpKind::of(spec.expect) {
            let name = dump.expect();
            if !kind.speaks_clang() {
                let reason =
                    format!("not supported by this compiler (`{name}` needs kiln or clang)");
                return (Status::Skip, Some(reason), String::new());
            }
            if self.cfg.oracle.is_none() {
                let reason = format!("no oracle (`{name}` is checked against the oracle's dump)");
                return (Status::Skip, Some(reason), String::new());
            }
        }
        if let Some(verdict) = self.prebuild_modules(&spec) {
            return verdict;
        }
        let outcome = match spec.expect {
            Expect::CompileOk => self.compile_ok(&spec),
            Expect::CompileFail => self.compile_fail(&spec),
            Expect::Run => self.run(&spec),
            Expect::Pp => self.pp(&spec),
            Expect::Ast => self.ast(&spec),
            Expect::Layout => self.dump(&spec, DumpKind::Layout),
            Expect::Vtables => self.dump(&spec, DumpKind::Vtables),
            Expect::Ir => self.dump(&spec, DumpKind::Ir),
            Expect::NoCrash => self.no_crash(&spec),
            Expect::Cfg => self.cfg_dump(&spec),
            Expect::Debugger => self.debugger(&spec),
        };
        let outcome = self.with_gcc_verdict(&spec, outcome);
        // Budgets are checked once the kind's own checks pass.
        let outcome = outcome.and_then(|()| self.check_budgets(&spec));
        let xfail = spec.xfail_reason(kind).map(str::to_owned);
        match (outcome, xfail) {
            (Ok(()), None) => (Status::Pass, None, String::new()),
            (Ok(()), Some(r)) => (
                Status::XPass,
                Some(r),
                "test passed but is marked xfail for this compiler; remove the marker".into(),
            ),
            (Err(Problem::Fail(d)), None) => (Status::Fail, None, d),
            (Err(Problem::Fail(d)), Some(r)) => (Status::XFail, Some(r), d),
            (Err(Problem::Broken(d)), r) => (Status::Error, r, d),
            (Err(Problem::Killed { mib, details }), _) => {
                (Status::Killed, Some(format!("memory limit {mib} MiB")), details)
            }
        }
    }

    /// The clangs `spec` runs, for `requires: clang>=`: the compiler under
    /// test when it is clang, and the oracle when it is clang and the test
    /// compares with it.
    fn clangs_run(&self, spec: &TestSpec) -> Vec<directives::ClangRun<'static>> {
        let mut out = Vec::new();
        if self.cfg.compiler_kind == CompilerKind::Clang {
            out.push(directives::ClangRun {
                role: "the compiler",
                version: self.cfg.compiler_clang,
            });
        }
        if spec.oracle && self.cfg.oracle.is_some() && self.cfg.oracle_kind == CompilerKind::Clang {
            out.push(directives::ClangRun { role: "the oracle", version: self.cfg.oracle_clang });
        }
        out
    }

    /// The LLVM backends that consume `spec`'s IR, for `requires: llvm>=`:
    /// kiln's (`llvm_dir`) when kiln compiles it, and every clang it runs
    /// (a clang's major is its LLVM's).
    fn llvms_run(&self, spec: &TestSpec) -> Vec<directives::LlvmRun<'static>> {
        let kiln_oracle =
            spec.oracle && self.cfg.oracle.is_some() && self.cfg.oracle_kind == CompilerKind::Kiln;
        let mut out = Vec::new();
        if self.cfg.compiler_kind == CompilerKind::Kiln || kiln_oracle {
            out.push(directives::LlvmRun {
                role: "kiln's backend",
                major: Some(self.cfg.llvm_major),
            });
        }
        out.extend(
            self.clangs_run(spec)
                .into_iter()
                .map(|c| directives::LlvmRun { role: c.role, major: c.version.map(|v| v[0]) }),
        );
        out
    }

    /// `<program> [-x lang] -std=<std> <mode...> <args...> <file> [-o out]`.
    fn command(
        &self,
        program: &Path,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Command {
        let mut c = Command::new(program);
        c.current_dir(&self.cfg.root);
        if self.is_kiln(program) {
            c.env("KILN_LLVM_DIR", &self.cfg.llvm_dir);
        }
        match (self.case.lang, self.case.is_header) {
            (Lang::C, _) => {
                c.args(["-x", "c"]);
            }
            (Lang::Cxx, true) => {
                c.args(["-x", "c++"]);
            }
            (Lang::Cxx, false) => {}
        }
        c.arg(format!("-std={}", spec.std));
        c.args(mode);
        if let Some(n) = self.clang_compat(program, spec) {
            c.arg(format!("-fclang-compat={n}"));
        }
        if program == self.cfg.compiler {
            c.args(self.meta_header_flags(true));
        }
        c.args(spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))));
        c.args(self.module_flags(program, spec, spec.modules.len()));
        c.arg(&self.case.name);
        if let Some(o) = out {
            c.arg("-o").arg(o);
        }
        c
    }

    /// The `module:` source of `path` (relative to the test's directory).
    fn module_source(&self, path: &Path) -> PathBuf {
        self.case.path.parent().unwrap_or(&self.cfg.root).join(path)
    }

    /// Separate compilation of the module units (spec/modules.md §11.7):
    /// `run` tests with `module:` or `unit:` lines, and `two-step`.
    fn two_step(spec: &TestSpec) -> bool {
        spec.two_step
            || (spec.expect == Expect::Run && (!spec.modules.is_empty() || !spec.units.is_empty()))
    }

    /// The `-fmodule-file=<name>=<file>` flags of the first `n` `module:`
    /// lines for `program`: kiln builds each from its source unless the
    /// test is [two-step](Self::two_step); otherwise `program` reads the
    /// module file [`Self::build_modules`] wrote.
    fn module_flags(&self, program: &Path, spec: &TestSpec, n: usize) -> Vec<std::ffi::OsString> {
        let source = self.is_kiln(program) && !Self::two_step(spec);
        let oracle = program != self.cfg.compiler;
        spec.modules[..n]
            .iter()
            .map(|(name, path)| {
                let file = if source { self.module_source(path) } else { self.bmi(name, oracle) };
                let mut f = std::ffi::OsString::from(format!("-fmodule-file={name}="));
                f.push(file);
                f
            })
            .collect()
    }

    /// The module file of unit `name` (the oracle's apart).
    fn bmi(&self, name: &str, oracle: bool) -> PathBuf {
        let stem = name.replace(':', "-");
        self.dir.join(if oracle { format!("oracle-{stem}.pcm") } else { format!("{stem}.pcm") })
    }

    /// The object of module unit `name` or of `unit:` file `i`.
    fn unit_object(&self, stem: &str, oracle: bool) -> PathBuf {
        self.dir.join(if oracle { format!("oracle-{stem}.o") } else { format!("{stem}.o") })
    }

    /// Compiles each `module:` unit in order, with the test's `-std` and
    /// `args:` and the earlier units' module files, into the scratch
    /// directory: a module file (`--precompile -x c++-module`), and with
    /// `objects` also its object (kiln: `-c -fmodule-output=`; clang: `-c`
    /// of the module file, its two-phase build). Kiln builds nothing
    /// unless the test is [two-step](Self::two_step). Returns the objects.
    /// A unit the compiler under test rejects fails the test; one the
    /// oracle rejects makes it broken.
    fn build_modules(
        &mut self,
        program: &Path,
        spec: &TestSpec,
        objects: bool,
    ) -> Result<Vec<PathBuf>, Problem> {
        if spec.modules.is_empty() || (self.is_kiln(program) && !Self::two_step(spec)) {
            return Ok(Vec::new());
        }
        self.scratch()?;
        let oracle = program != self.cfg.compiler;
        let who = if oracle { "the oracle" } else { "the compiler" };
        let kiln = self.is_kiln(program);
        let mut out = Vec::new();
        for (i, (name, path)) in spec.modules.iter().enumerate() {
            let src = self.module_source(path);
            if !src.is_file() {
                return Err(Problem::Broken(format!(
                    "`module: {name}={}`: no such file {}",
                    path.display(),
                    src.display()
                )));
            }
            let bmi = self.bmi(name, oracle);
            let obj = self.unit_object(&name.replace(':', "-"), oracle);
            let base = |c: &mut Command| {
                c.current_dir(&self.cfg.root);
                if kiln {
                    c.env("KILN_LLVM_DIR", &self.cfg.llvm_dir);
                }
                c.arg(format!("-std={}", spec.std));
                if let Some(n) = self.clang_compat(program, spec) {
                    c.arg(format!("-fclang-compat={n}"));
                }
                c.args(
                    spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))),
                );
                c.args(self.module_flags(program, spec, i));
            };
            let mut steps = Vec::new();
            let mut c = Command::new(program);
            base(&mut c);
            if kiln && objects {
                let mut f = std::ffi::OsString::from("-fmodule-output=");
                f.push(&bmi);
                c.arg(f).args(["-c", "-x", "c++-module"]).arg(&src).arg("-o").arg(&obj);
                steps.push(c);
            } else {
                c.args(["--precompile", "-x", "c++-module"]).arg(&src).arg("-o").arg(&bmi);
                steps.push(c);
                if objects {
                    let mut c = Command::new(program);
                    base(&mut c);
                    c.arg("-c").arg(&bmi).arg("-o").arg(&obj);
                    steps.push(c);
                }
            }
            for c in &steps {
                let o = self.exec(who, c, self.cfg.compile_timeout)?;
                let crashed = o.stderr_str().contains("panicked at");
                if !o.success() || crashed {
                    let msg = format!(
                        "{who} could not build module `{name}` ({})\n{}",
                        o.describe_end(),
                        indent(&o.stderr_str())
                    );
                    return Err(if oracle { Problem::Broken(msg) } else { Problem::Fail(msg) });
                }
            }
            if objects {
                out.push(obj);
            }
        }
        Ok(out)
    }

    /// A [two-step](Self::two_step) `run` build by `program` into `exe`:
    /// the module units ([`Self::build_modules`]), the test file and each
    /// `unit:` file to objects against the module files, then one link.
    /// Returns the test file's compile (its diagnostics are checked), or
    /// that of the first step that failed.
    fn build_two_step(
        &mut self,
        program: &Path,
        spec: &TestSpec,
        exe: &Path,
    ) -> Result<Output, Problem> {
        let oracle = program != self.cfg.compiler;
        let who = if oracle { "the oracle" } else { "the compiler" };
        let mut objects = self.build_modules(program, spec, true)?;
        let test_obj = self.unit_object("test", oracle);
        let main = self.command(program, spec, &["-c"], Some(&test_obj));
        let main_out = self.exec(who, &main, self.cfg.compile_timeout)?;
        if !main_out.success() {
            return Ok(main_out);
        }
        for (i, unit) in spec.units.iter().enumerate() {
            let src = self.module_source(unit);
            let obj = self.unit_object(&format!("unit{i}"), oracle);
            let mut c = Command::new(program);
            c.current_dir(&self.cfg.root);
            if self.is_kiln(program) {
                c.env("KILN_LLVM_DIR", &self.cfg.llvm_dir);
            }
            c.arg(format!("-std={}", spec.std));
            if let Some(n) = self.clang_compat(program, spec) {
                c.arg(format!("-fclang-compat={n}"));
            }
            c.args(spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))));
            c.args(self.module_flags(program, spec, spec.modules.len()));
            c.arg("-c").arg(&src).arg("-o").arg(&obj);
            let o = self.exec(who, &c, self.cfg.compile_timeout)?;
            if !o.success() {
                return Ok(o);
            }
            objects.push(obj);
        }
        objects.push(test_obj);
        let mut link = Command::new(program);
        link.current_dir(&self.cfg.root);
        if self.is_kiln(program) {
            link.env("KILN_LLVM_DIR", &self.cfg.llvm_dir);
        }
        link.arg("-Qunused-arguments");
        link.args(spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))));
        link.args(&objects).arg("-o").arg(exe);
        let o = self.exec(who, &link, self.cfg.compile_timeout)?;
        if !o.success() {
            return Ok(o);
        }
        Ok(main_out)
    }

    /// The `-fclang-compat=<major>` kiln is run with: the oracle clang's
    /// major version, so kiln answers the feature queries as the oracle
    /// does (`spec/harness.md` "Oracle"; WP-263). `None` when
    /// `program` is not kiln or no clang oracle is configured. A test
    /// whose `requires: clang>=N` / `clang<N` the oracle does not meet
    /// runs kiln alone, imitating the clang it requires: the nearest
    /// level kiln has on that side ([`crate::oracle::SUPPORTED`]).
    fn clang_compat(&self, program: &Path, spec: &TestSpec) -> Option<u32> {
        let kiln = program == self.cfg.compiler && self.cfg.compiler_kind == CompilerKind::Kiln;
        if !kiln || self.cfg.oracle_kind != CompilerKind::Clang {
            return None;
        }
        let oracle = self.cfg.oracle_clang.map(|v| v[0])?;
        Some(compat_for(oracle, spec.requires_clang, spec.requires_clang_below))
    }

    /// The header-differential mode's flags, ahead of the test's `args:`:
    /// the compatibility headers' directory first on the include path (so
    /// `"kiln_meta.h"` is theirs) and, for kiln (`toolchain`), the GCC
    /// installation whose libstdc++ has `<meta>`. Empty outside the mode.
    fn meta_header_flags(&self, toolchain: bool) -> Vec<String> {
        let (Some(dir), Some(gcc)) = (&self.cfg.meta_header, &self.cfg.gcc_toolchain) else {
            return Vec::new();
        };
        let mut v = vec![format!("-I{}", dir.display())];
        if toolchain {
            v.push(format!("--gcc-toolchain={}", gcc.display()));
        }
        v
    }

    /// The value of an `args:` placeholder (`spec/harness.md` "Placeholders").
    fn placeholder(&self, name: &str) -> Option<String> {
        let path = match name {
            "gcc_toolchain" => self.cfg.gcc_toolchain.clone()?,
            "repo_root" => self.cfg.root.clone(),
            "resource_dir" => self.cfg.root.join("resource"),
            "test_dir" => self.case.path.parent()?.to_path_buf(),
            _ => return None,
        };
        Some(path.to_string_lossy().into_owned())
    }

    /// Whether `program` is kiln (as the compiler under test or the oracle).
    fn is_kiln(&self, program: &Path) -> bool {
        (program == self.cfg.compiler && self.cfg.compiler_kind == CompilerKind::Kiln)
            || (self.cfg.oracle.as_deref() == Some(program)
                && self.cfg.oracle_kind == CompilerKind::Kiln)
    }

    /// The cap of a compile by `program`: the test's cap, plus the stack
    /// kiln reserves when `program` is kiln (`spec/harness.md` "Resource
    /// limits").
    fn compile_cap(&self, program: &Path, cmd: &Command) -> Option<MemLimit> {
        let cap = self.mem_limit?;
        if !self.is_kiln(program) {
            return Some(cap);
        }
        let args: Vec<_> = cmd.get_args().collect();
        Some(cap.plus(process::kiln_stack_reservation_mib(&args)))
    }

    /// Runs a compiler (`who`, for messages) under its cap. Failing to start
    /// it is broken; running out of memory under the cap is KILLED.
    fn exec(&mut self, who: &str, cmd: &Command, timeout: Duration) -> Result<Output, Problem> {
        let cap = self.compile_cap(Path::new(cmd.get_program()), cmd);
        let shown = Shown(cmd).to_string();
        let r = self.run_capped(&shown, cmd, None, cap, timeout);
        let o = r.map_err(|e| Problem::Broken(format!("could not run `{shown}`: {e}")))?;
        self.check_capped(who, &o, cap)?;
        Ok(o)
    }

    /// Runs `cmd` under `cap`, logging it as `shown`.
    fn run_capped(
        &mut self,
        shown: &str,
        cmd: &Command,
        stdin: Option<&[u8]>,
        cap: Option<MemLimit>,
        timeout: Duration,
    ) -> std::io::Result<Output> {
        let mut capped = process::limited(cmd, cap);
        let r = process::run_with_input(&mut capped, stdin, timeout);
        let limit = cap.map_or_else(String::new, |c| format!(" [mem<={} MiB]", c.mib));
        match &r {
            Ok(o) => self.log.push(format!("$ {shown}{limit}  # {}", o.describe_end())),
            Err(e) => self.log.push(format!("$ {shown}{limit}  # failed to start: {e}")),
        }
        r
    }

    /// KILLED when `o` ran out of memory under `cap`; broken when the cap
    /// could not be applied.
    fn check_capped(&self, who: &str, o: &Output, cap: Option<MemLimit>) -> Check {
        let Some(cap) = cap else { return Ok(()) };
        let stderr = o.stderr_str();
        if o.code() == Some(MemLimit::ULIMIT_FAILED)
            && stderr.lines().any(|l| l.starts_with("kiln-mem-limit:") && l.contains("ulimit"))
        {
            return Err(Problem::Broken(format!(
                "cannot apply the memory limit of {} MiB to {who}\n{}",
                cap.mib,
                indent(&stderr)
            )));
        }
        if !o.out_of_memory() {
            return Ok(());
        }
        let mib = self.mem_limit.map_or(cap.mib, |l| l.mib);
        let allowance = if cap.mib > mib {
            format!(" + {} MiB kiln stack reservation", cap.mib - mib)
        } else {
            String::new()
        };
        Err(Problem::Killed {
            mib,
            details: format!(
                "{who} ran out of memory under the {mib} MiB address-space cap{allowance} ({})\n{}",
                o.describe_end(),
                indent(&tail(&stderr, 20))
            ),
        })
    }

    /// Runs the compiler under test and rejects timeouts and crashes.
    fn compile(
        &mut self,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Result<Output, Problem> {
        let mut cmd = self.command(&self.cfg.compiler, spec, mode, out);
        if spec.inst_report.is_some() {
            cmd.arg("-fkiln-instantiation-report=json");
        }
        if !spec.oven.is_empty() {
            // Only kiln gets here (`evaluate` skips the rest). A stale report
            // must not pass for this compile's.
            let report = self.report_path()?;
            if report.exists() {
                fs::remove_file(&report).map_err(|e| {
                    Problem::Broken(format!("cannot remove {}: {e}", report.display()))
                })?;
            }
            let mut flag = std::ffi::OsString::from(oven_report::FLAG);
            flag.push(&report);
            cmd.arg(flag);
        }
        let o = self.exec("the compiler", &cmd, self.cfg.compile_timeout)?;
        let stderr = o.stderr_str();
        if spec.inst_report.is_some() {
            self.inst_report = Some(inst_report::extract(&stderr));
        }
        let crashed = o.code().is_none()
            || stderr.contains("panicked at")
            || stderr.contains("PLEASE submit a bug report");
        if crashed {
            let what = if o.timed_out() { "hung" } else { "crashed" };
            return Err(Problem::Fail(format!(
                "compiler {what} ({})\n{}",
                o.describe_end(),
                indent(&stderr)
            )));
        }
        Ok(o)
    }

    /// Runs the oracle compiler; any failure means the test is broken.
    fn oracle_compile(
        &mut self,
        oracle: &Path,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Result<Output, Problem> {
        self.build_modules(oracle, spec, false)?;
        let cmd = self.command(oracle, spec, mode, out);
        let o = self.exec("the oracle", &cmd, self.cfg.compile_timeout)?;
        if o.success() {
            Ok(o)
        } else if o.timed_out() {
            Err(Problem::Broken(format!(
                "the oracle ({}) hung compiling the test ({})",
                oracle.display(),
                o.describe_end()
            )))
        } else {
            Err(Problem::Broken(format!(
                "the oracle ({}) rejected the test ({})\n{}",
                oracle.display(),
                o.describe_end(),
                indent(&o.stderr_str())
            )))
        }
    }

    fn in_test_file(&self, file: &str) -> bool {
        if file == self.case.name {
            return true;
        }
        let p = self.cfg.root.join(file);
        fs::canonicalize(p).is_ok_and(|p| p == self.case.path)
    }

    fn check_diags(&self, spec: &TestSpec, diags: &[Diagnostic]) -> Check {
        let clang = self.cfg.compiler_kind == CompilerKind::Clang;
        let directives = diag_parse::expand_clang_dups(&spec.diags, clang);
        let report = diag_parse::match_diags(
            &directives,
            diags,
            &|f| self.in_test_file(f),
            spec.strict_diags,
        );
        if report.is_ok() {
            Ok(())
        } else {
            Err(Problem::Fail(diag_parse::explain(&report, &directives, diags)))
        }
    }

    fn compile_ok(&mut self, spec: &TestSpec) -> Check {
        if spec.verify {
            return self.verify(spec);
        }
        let o = self.compile(spec, &["-fsyntax-only"], None)?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "expected compile-ok, but the compiler failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))
    }

    /// `verify`: the compiler's `-Xclang -verify` check of the test's
    /// `expected-*` comments must pass (exit 0), and so must the oracle's
    /// (else the comments themselves are wrong: ERROR). kiln accepts
    /// `-Xclang -verify` as clang does.
    fn verify(&mut self, spec: &TestSpec) -> Check {
        const MODE: &[&str] = &["-fsyntax-only", "-Xclang", "-verify"];
        let o = self.compile(spec, MODE, None)?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "the compiler's -verify check failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        if spec.oracle
            && let Some(oracle) = self.cfg.oracle.clone()
            && (self.cfg.compiler_kind == CompilerKind::Kiln || oracle != self.cfg.compiler)
        {
            self.oracle_compile(&oracle, spec, MODE, None)?;
        }
        Ok(())
    }

    fn compile_fail(&mut self, spec: &TestSpec) -> Check {
        let o = self.compile(spec, &["-fsyntax-only"], None)?;
        let stderr = o.stderr_str();
        if o.success() {
            return Err(Problem::Fail(format!(
                "expected compile-fail, but compilation succeeded\n{}",
                indent(&stderr)
            )));
        }
        let diags = diag_parse::parse(&stderr);
        let has_error = diags.iter().any(|d| {
            Severity::Error.accepts(d.severity)
                && d.loc.as_ref().is_some_and(|l| self.in_test_file(&l.file))
        });
        if !has_error {
            return Err(Problem::Fail(format!(
                "the compiler failed ({}) without an error located in the test file\n{}",
                o.describe_end(),
                indent(&stderr)
            )));
        }
        self.check_diags(spec, &diags)
    }

    /// `expect: no-crash`: the compile ends normally, with or without
    /// errors (exit 0 or 1). A crash or hang is already a FAIL in
    /// [`Self::compile`]; any other exit code (70, 101, ...) is one too.
    /// The `diag:` lines are still checked.
    fn no_crash(&mut self, spec: &TestSpec) -> Check {
        // No error limit by default: a crash must not hide behind the 20th
        // error (WP-141's `<string>` panic did). It precedes the test's
        // `args:`, so an explicit `-ferror-limit=N` there still wins.
        let o = self.compile(spec, &["-fsyntax-only", "-ferror-limit=0"], None)?;
        let stderr = o.stderr_str();
        if !matches!(o.code(), Some(0 | 1)) {
            return Err(Problem::Fail(format!(
                "expected no-crash (exit code 0 or 1), but the compiler ended with {}\n{}",
                o.describe_end(),
                indent(&stderr)
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&stderr))
    }

    fn scratch(&self) -> Result<&Path, Problem> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| Problem::Broken(format!("cannot create {}: {e}", self.dir.display())))?;
        Ok(&self.dir)
    }

    fn report_path(&self) -> Result<PathBuf, Problem> {
        Ok(self.scratch()?.join("constexpr-report.jsonl"))
    }

    /// Reads the constexpr report of the compile and checks the `oven:`
    /// budgets. A missing or malformed report is kiln's fault: FAIL.
    /// The `oven:` and `inst-report:` budgets. The header-differential
    /// mode has none (the real <meta> evaluates thousands of its own
    /// constants).
    fn check_budgets(&mut self, spec: &TestSpec) -> Check {
        if self.cfg.meta_header.is_some() {
            return Ok(());
        }
        self.check_oven(spec)?;
        self.check_inst_report(spec)
    }

    fn check_oven(&mut self, spec: &TestSpec) -> Check {
        if spec.oven.is_empty() {
            return Ok(());
        }
        let path = self.report_path()?;
        let text = fs::read_to_string(&path).map_err(|e| {
            Problem::Fail(format!(
                "oven: the compiler wrote no readable constexpr report at {} ({e})",
                path.display()
            ))
        })?;
        let report = Report::parse(&text).map_err(|e| {
            Problem::Fail(format!("oven: malformed constexpr report {}: {e}", path.display()))
        })?;
        self.log.push(format!("# constexpr report total: {}", report.total.describe()));
        oven_report::check_budgets(&report, &spec.oven, &|f| self.in_test_file(f))
            .map_err(Problem::Fail)
    }

    /// Compares the compile's instantiation report with the
    /// `inst-report:` golden, timings masked (`--update` rewrites it).
    fn check_inst_report(&mut self, spec: &TestSpec) -> Check {
        let Some(rel) = &spec.inst_report else { return Ok(()) };
        let path = self.case.path.parent().unwrap_or(Path::new(".")).join(rel);
        let got = self.inst_report.take().unwrap_or_default();
        if self.cfg.update {
            fs::write(&path, &got).map_err(|e| {
                Problem::Broken(format!("cannot write inst-report {}: {e}", path.display()))
            })?;
            self.updated = true;
            return Ok(());
        }
        let want = fs::read_to_string(&path).map_err(|e| {
            Problem::Broken(format!("cannot read inst-report {}: {e}", path.display()))
        })?;
        let want = inst_report::mask(&want);
        if want == got {
            Ok(())
        } else {
            Err(Problem::Fail(format!(
                "inst-report: the instantiation report differs from {}\nexpected:\n{}got:\n{}",
                path.display(),
                indent(&want),
                indent(&got)
            )))
        }
    }

    /// The stdin bytes of a `run` test (empty without `run-stdin*`).
    fn stdin_bytes(&self, spec: &TestSpec) -> Result<Option<Vec<u8>>, Problem> {
        Ok(match &spec.stdin {
            None => None,
            Some(StdinInput::Inline(s)) => Some(s.clone().into_bytes()),
            Some(StdinInput::File(p)) => {
                let path = self.case.path.parent().unwrap_or(Path::new(".")).join(p);
                Some(fs::read(&path).map_err(|e| {
                    Problem::Broken(format!("cannot read run-stdin-file {}: {e}", path.display()))
                })?)
            }
        })
    }

    /// Runs a built binary in the scratch directory with the test's
    /// arguments, environment and stdin, under the test's cap. Failing to
    /// start the test's binary fails the test, the oracle's makes it broken.
    fn execute_binary(
        &mut self,
        exe: &Path,
        spec: &TestSpec,
        stdin: Option<&[u8]>,
        oracle: bool,
    ) -> Result<Output, Problem> {
        let mut cmd = Command::new(exe);
        cmd.current_dir(&self.dir).args(&spec.run_args);
        for (k, v) in &spec.env {
            cmd.env(k, v);
        }
        let mut shown = Shown(&cmd).to_string();
        if !spec.env.is_empty() {
            let vars: Vec<String> = spec.env.iter().map(|(k, v)| format!("{k}={v:?}")).collect();
            shown = format!("{} {shown}", vars.join(" "));
        }
        if stdin.is_some() {
            shown.push_str(" < (stdin)");
        }
        let cap = self.mem_limit;
        let o = self.run_capped(&shown, &cmd, stdin, cap, self.cfg.run_timeout).map_err(|e| {
            let d = format!("could not run `{shown}`: {e}");
            if oracle { Problem::Broken(d) } else { Problem::Fail(d) }
        })?;
        let who = if oracle { "the oracle's binary" } else { "the test binary" };
        self.check_capped(who, &o, cap)?;
        Ok(o)
    }

    /// Builds a `run` test's binary `exe` with the compiler under test,
    /// in one command or [two steps](Self::two_step); the output returned
    /// is the test file's compile's.
    fn compile_run_test(&mut self, spec: &TestSpec, exe: &Path) -> Result<Output, Problem> {
        if !Self::two_step(spec) {
            return self.compile(spec, &[], Some(exe));
        }
        let o = self.build_two_step(&self.cfg.compiler.clone(), spec, exe)?;
        if o.code().is_none() || o.stderr_str().contains("panicked at") {
            let what = if o.timed_out() { "hung" } else { "crashed" };
            return Err(Problem::Fail(format!(
                "compiler {what} ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        Ok(o)
    }

    /// Builds a `run` test's binary `exe` with `oracle`; a rejection is
    /// [`Problem::Broken`].
    fn compile_run_oracle(&mut self, oracle: &Path, spec: &TestSpec, exe: &Path) -> Check {
        if !Self::two_step(spec) {
            return self.oracle_compile(oracle, spec, &[], Some(exe)).map(drop);
        }
        let o = self.build_two_step(oracle, spec, exe)?;
        if o.success() {
            return Ok(());
        }
        Err(Problem::Broken(format!(
            "the oracle ({}) rejected the test ({})\n{}",
            oracle.display(),
            o.describe_end(),
            indent(&o.stderr_str())
        )))
    }

    fn run(&mut self, spec: &TestSpec) -> Check {
        let exe = self.scratch()?.join("test.out");
        let o = self.compile_run_test(spec, &exe)?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "compiling the test failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))?;
        let stdin = self.stdin_bytes(spec)?;
        let got = self.execute_binary(&exe, spec, stdin.as_deref(), false)?;
        if got.code().is_none() {
            return Err(Problem::Fail(format!(
                "test binary {}\nstdout:\n{}stderr:\n{}",
                got.describe_end(),
                indent(&got.stdout_str()),
                indent(&got.stderr_str())
            )));
        }
        let mut failures = String::new();
        if let Some(expected) = &spec.stdout {
            let want = match expected {
                StdoutExpect::Inline(s) => s.clone(),
                StdoutExpect::File(p) => {
                    let path = self.case.path.parent().unwrap_or(Path::new(".")).join(p);
                    fs::read_to_string(&path).map_err(|e| {
                        Problem::Broken(format!("cannot read stdout-file {}: {e}", path.display()))
                    })?
                }
            };
            if got.stdout_str() != want {
                let _ = write!(
                    failures,
                    "stdout differs from the expectation (-expected +actual):\n{}",
                    indent(&line_diff(&want, &got.stdout_str()))
                );
            }
        }
        let want_exit = spec.exit.unwrap_or(0);
        if got.code() != Some(want_exit) {
            let _ =
                writeln!(failures, "expected exit code {want_exit}, got {}", got.describe_end());
        }
        if spec.oracle
            && let Some(oracle) = self.cfg.oracle.clone()
        {
            let oexe = self.dir.join("oracle.out");
            self.compile_run_oracle(&oracle, spec, &oexe)?;
            let theirs = self.execute_binary(&oexe, spec, stdin.as_deref(), true)?;
            if theirs.code().is_none() {
                // The oracle's binary is the reference; if it hangs or
                // crashes, the test itself is broken.
                return Err(Problem::Broken(format!(
                    "the oracle's binary {}\nstdout:\n{}stderr:\n{}",
                    theirs.describe_end(),
                    indent(&theirs.stdout_str()),
                    indent(&theirs.stderr_str())
                )));
            }
            if theirs.stdout != got.stdout {
                let _ = write!(
                    failures,
                    "stdout differs from the oracle (-oracle +actual):\n{}",
                    indent(&line_diff(&theirs.stdout_str(), &got.stdout_str()))
                );
            }
            if theirs.code() != got.code() {
                let _ = writeln!(
                    failures,
                    "exit differs from the oracle: oracle {}, actual {}",
                    theirs.describe_end(),
                    got.describe_end()
                );
            }
        }
        if let Some(gcc) = self.gcc_applies(spec) {
            self.gcc_run(&gcc, spec, stdin.as_deref(), &got, &mut failures)?;
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }

    /// `expect: debugger`: compile and link at `-O0 -g`, then run each
    /// script in batch mode (`gdb -batch -nx -x`, `lldb -b -s`) with cwd =
    /// the scratch dir; each `*-check` must be a substring of a later
    /// output line (stdout, then stderr) than the previous check's.
    fn debugger(&mut self, spec: &TestSpec) -> Check {
        let exe = self.scratch()?.join("test.out");
        let o = self.compile(spec, &["-O0", "-g"], Some(&exe))?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "compiling the test failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))?;
        let mut failures = String::new();
        for (name, sc) in [("gdb", &spec.gdb), ("lldb", &spec.lldb)] {
            if sc.commands.is_empty() {
                continue;
            }
            let Some(prog) = debugger_program(name) else { continue };
            let script = self.dir.join(format!("{name}.cmd"));
            let mut text = sc.commands.join("\n");
            text.push('\n');
            fs::write(&script, text)
                .map_err(|e| Problem::Broken(format!("cannot write {}: {e}", script.display())))?;
            let mut cmd = Command::new(&prog);
            cmd.current_dir(&self.dir);
            // No symbol-server lookups: the test binary has all its debug
            // information, and a network round trip is neither fast nor
            // deterministic.
            cmd.env_remove("DEBUGINFOD_URLS");
            if name == "gdb" {
                cmd.args(["-batch", "-nx", "-x"]).arg(&script).arg(&exe);
            } else {
                cmd.args(["-b", "-s"]).arg(&script).arg(&exe);
            }
            let shown = Shown(&cmd).to_string();
            let got = self
                .run_capped(&shown, &cmd, None, None, self.cfg.run_timeout.max(DEBUGGER_TIMEOUT))
                .map_err(|e| Problem::Broken(format!("could not run `{shown}`: {e}")))?;
            let out = format!("{}{}", got.stdout_str(), got.stderr_str());
            if got.timed_out() {
                let _ =
                    write!(failures, "{name}: {}; output:\n{}", got.describe_end(), indent(&out));
                continue;
            }
            if let Some(miss) = first_missing_check(&out, &sc.checks) {
                let _ = write!(
                    failures,
                    "{name}: no line contains `{miss}` (after the earlier checks); output:\n{}",
                    indent(&out)
                );
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }

    fn pp(&mut self, spec: &TestSpec) -> Check {
        let mode: &[&str] = if spec.pp_linemarkers { &["-E"] } else { &["-E", "-P"] };
        let how = self.cfg.pp_compare.unwrap_or(spec.pp_compare);
        let compare = |want: &str, got: &str, labels| {
            pp_diff(how, &spec.std, spec.pp_linemarkers, want, got, labels)
        };
        let rule = match how {
            PpCompare::Tokens => "token-wise",
            PpCompare::Text => "whitespace-normalized text",
        };
        let o = self.compile(spec, mode, None)?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "preprocessing failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))?;
        let got = o.stdout_str();
        let mut failures = String::new();
        let mut golden = self.case.path.clone().into_os_string();
        golden.push(".expected");
        let golden = PathBuf::from(golden);
        if golden.is_file() {
            let want = fs::read_to_string(&golden)
                .map_err(|e| Problem::Broken(format!("cannot read {}: {e}", golden.display())))?;
            let diff = compare(&want, &got, ("expected", "actual"));
            if self.cfg.update {
                // Only a real difference rewrites: an equivalent output
                // spelled differently (another compiler's spacing) is kept.
                if diff.is_some() {
                    fs::write(&golden, &got).map_err(|e| {
                        Problem::Broken(format!("cannot write {}: {e}", golden.display()))
                    })?;
                    self.updated = true;
                }
            } else if let Some(d) = diff {
                let _ = write!(
                    failures,
                    "output differs from {} ({rule}):\n{}",
                    golden.file_name().unwrap_or_default().to_string_lossy(),
                    indent(&d)
                );
            }
        }
        if spec.oracle
            && let Some(oracle) = self.cfg.oracle.clone()
        {
            let theirs = self.oracle_compile(&oracle, spec, mode, None)?.stdout_str();
            if let Some(d) = compare(&theirs, &got, ("oracle", "actual")) {
                let _ =
                    write!(failures, "output differs from the oracle ({rule}):\n{}", indent(&d));
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }
}

impl Test<'_> {
    /// `expect: ast` (`spec/ast-dump.md` §7): kiln's `-fdump-ast`, or clang's
    /// JSON dump normalized by `xtask clang-ast`, against the golden.
    fn ast(&mut self, spec: &TestSpec) -> Check {
        let mut golden = self.case.path.clone().into_os_string();
        golden.push(".expected");
        let golden = PathBuf::from(golden);
        let golden_name = golden.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let want = match fs::read_to_string(&golden) {
            Ok(w) => w,
            Err(e) => {
                return Err(Problem::Broken(format!(
                    "cannot read the golden {golden_name}: {e} (an `ast` test needs one; \
                     `cargo xtask clang-ast <test>` prints clang's dump in the kiln format)"
                )));
            }
        };
        let got = if self.cfg.compiler_kind == CompilerKind::Kiln {
            let filter = spec.ast_filter.as_ref().map(|f| format!("-fdump-ast-filter={f}"));
            let dump = if spec.ast_implicit { "-fdump-ast=implicit" } else { "-fdump-ast" };
            let mut mode = vec!["-fsyntax-only", dump];
            mode.extend(filter.as_deref());
            let o = self.compile(spec, &mode, None)?;
            self.ast_compiled(spec, &o)?;
            o.stdout_str()
        } else {
            let compiler = self.cfg.compiler.clone();
            self.clang_ast(&compiler, spec, false)?
        };
        let mut failures = String::new();
        if let Some(d) = ast_diff(&want, &got) {
            if self.cfg.update {
                fs::write(&golden, &got).map_err(|e| {
                    Problem::Broken(format!("cannot write {}: {e}", golden.display()))
                })?;
                self.updated = true;
            } else {
                let _ = write!(failures, "dump differs from {golden_name}:\n{}", indent(&d));
            }
        }
        if spec.oracle
            && let Some(oracle) = self.cfg.oracle.clone()
            && (self.cfg.compiler_kind == CompilerKind::Kiln || oracle != self.cfg.compiler)
        {
            let theirs = self.clang_ast(&oracle, spec, true)?;
            if let Some(d) = ast_diff(&want, &theirs) {
                let _ = write!(
                    failures,
                    "the oracle's normalized dump differs from {golden_name}:\n{}",
                    indent(&d)
                );
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }

    /// `expect: cfg` (spec/harness.md "cfg tests"): kiln's
    /// `-fkiln-dump-cfg` against the golden `<test>.expected`, and the
    /// golden's shapes against the oracle's `debug.DumpCFG` (or, with clang
    /// as the compiler under test, against its own), except for the
    /// functions `cfg-diverges` names.
    fn cfg_dump(&mut self, spec: &TestSpec) -> Check {
        let mut golden = self.case.path.clone().into_os_string();
        golden.push(".expected");
        let golden = PathBuf::from(golden);
        let golden_name = golden.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let read = fs::read_to_string(&golden);
        let skip: Vec<&str> = spec.cfg_diverges.iter().map(|(f, _)| f.as_str()).collect();
        let mut failures = String::new();
        let want = if self.cfg.compiler_kind == CompilerKind::Kiln {
            let o = self.compile(spec, &["-fsyntax-only", "-fkiln-dump-cfg"], None)?;
            self.ast_compiled(spec, &o)?;
            let got = o.stdout_str();
            if self.cfg.update && read.as_ref().map_or(true, |w| ast_diff(w, &got).is_some()) {
                fs::write(&golden, &got).map_err(|e| {
                    Problem::Broken(format!("cannot write {}: {e}", golden.display()))
                })?;
                self.updated = true;
            } else {
                match &read {
                    Ok(w) => {
                        if let Some(d) = ast_diff(w, &got) {
                            let _ = write!(
                                failures,
                                "dump differs from {golden_name}:\n{}",
                                indent(&d)
                            );
                        }
                    }
                    Err(e) => {
                        return Err(Problem::Broken(format!(
                            "cannot read the golden {golden_name}: {e} (a `cfg` test needs one; \
                             `--update` writes kiln's dump)"
                        )));
                    }
                }
            }
            got
        } else {
            let w = read.map_err(|e| {
                Problem::Broken(format!("cannot read the golden {golden_name}: {e}"))
            })?;
            let compiler = self.cfg.compiler.clone();
            let o = self.compile(spec, &CLANG_CFG_MODE, None)?;
            if !o.success() {
                return Err(Problem::Fail(format!(
                    "expected the CFG dump, but {} failed ({})",
                    compiler.display(),
                    o.describe_end()
                )));
            }
            if let Some(d) = super::cfg_shape::diff(&w, &o.stderr_str(), &skip, false) {
                let _ = write!(
                    failures,
                    "the compiler's graphs differ from {golden_name}:\n{}",
                    indent(&d)
                );
            }
            w
        };
        if self.cfg.compiler_kind == CompilerKind::Kiln
            && let Some(oracle) = self.cfg.oracle.clone()
        {
            let theirs = self.oracle_compile(&oracle, spec, &CLANG_CFG_MODE, None)?.stderr_str();
            if let Some(d) = super::cfg_shape::diff(&theirs, &want, &skip, true) {
                let _ = write!(
                    failures,
                    "the graphs differ from the oracle's ({}):\n{}",
                    oracle.display(),
                    indent(&d)
                );
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }

    /// The exit status and `diag:` checks of an `ast` compile.
    fn ast_compiled(&self, spec: &TestSpec, o: &Output) -> Check {
        if !o.success() {
            return Err(Problem::Fail(format!(
                "expected the AST dump, but the compiler failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))
    }

    /// clang's AST of the test in the kiln format (`xtask clang-ast`). As the
    /// compiler under test its exit status and diagnostics are checked; as
    /// the `oracle`, any failure means the test is broken. A construct the
    /// normalizer cannot map makes the test broken either way: it cannot be
    /// checked against clang.
    fn clang_ast(
        &mut self,
        clang: &Path,
        spec: &TestSpec,
        oracle: bool,
    ) -> Result<String, Problem> {
        let json_mode = ["-fsyntax-only", "-Xclang", "-ast-dump=json"];
        let json = if oracle {
            self.oracle_compile(clang, spec, &json_mode, None)?
        } else {
            let o = self.compile(spec, &json_mode, None)?;
            self.ast_compiled(spec, &o)?;
            o
        }
        .stdout_str();
        let opts = DumpOptions {
            filter: spec.ast_filter.clone(),
            implicit: spec.ast_implicit,
            ..DumpOptions::default()
        };
        let main_file = self.case.name.clone();
        let timeout = self.cfg.compile_timeout;
        let who = if oracle { "the oracle" } else { "the compiler" };
        let mut killed = None;
        let mut text = |extra: &[&str]| {
            let mut flags = vec!["-fsyntax-only", "-Xclang", "-ast-dump"];
            flags.extend_from_slice(extra);
            let cmd = self.command(clang, spec, &flags, None);
            let o = self.exec(who, &cmd, timeout).map_err(|p| {
                let d = p.text().to_owned();
                if let Problem::Killed { .. } = p {
                    killed = Some(p);
                }
                d
            })?;
            if o.success() {
                Ok(o.stdout_str())
            } else {
                Err(format!("clang's text dump failed ({})", o.describe_end()))
            }
        };
        let r = normalize(&json, &main_file, &opts, &mut text);
        if let Some(k) = killed {
            return Err(k);
        }
        r.map_err(|e| {
            Problem::Broken(format!(
                "cannot normalize the AST dump of {} (`xtask clang-ast`):\n{}",
                clang.display(),
                indent(&e)
            ))
        })
    }
}

/// clang's CFG dump (`debug.DumpCFG`, on stderr; warnings off so they do
/// not interleave).
const CLANG_CFG_MODE: [&str; 6] =
    ["-fsyntax-only", "-w", "-Xclang", "-analyze", "-Xclang", "-analyzer-checker=debug.DumpCFG"];

/// The dump an `expect: layout`, `expect: vtables` or `expect: ir` test
/// compares (`spec/harness.md` "Layout and vtable tests", "IR tests").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DumpKind {
    Layout,
    Vtables,
    Ir,
}

impl DumpKind {
    fn of(expect: Expect) -> Option<Self> {
        match expect {
            Expect::Layout => Some(Self::Layout),
            Expect::Vtables => Some(Self::Vtables),
            Expect::Ir => Some(Self::Ir),
            _ => None,
        }
    }

    /// The directive, for messages.
    fn expect(self) -> &'static str {
        match self {
            Self::Layout => "expect: layout",
            Self::Vtables => "expect: vtables",
            Self::Ir => "expect: ir",
        }
    }

    /// What the dump holds, for messages.
    fn noun(self) -> &'static str {
        match self {
            Self::Layout => "record layout",
            Self::Vtables => "vtable",
            Self::Ir => "function definition",
        }
    }

    /// The number of blocks of `dump`.
    fn count(self, dump: &str) -> usize {
        match self {
            Self::Layout => record_layout_blocks(dump).len(),
            Self::Vtables => vtable_blocks(dump).len(),
            Self::Ir => dump.lines().filter(|l| l.starts_with("define ")).count(),
        }
    }

    /// Compares `got` with the oracle's dump `want`; `None` when they agree.
    fn diff(self, want: &str, got: &str) -> Option<String> {
        match self {
            Self::Layout => layout_diff(want, got),
            Self::Vtables => vtables_diff(want, got),
            Self::Ir => ir_diff(want, got),
        }
    }
}

impl Test<'_> {
    /// `expect: layout` / `expect: vtables`: the compiler's dump against the
    /// oracle's, in kiln's or clang's spelling of the dump flags. Only run
    /// with an oracle (`evaluate` skips the test otherwise).
    fn dump(&mut self, spec: &TestSpec, kind: DumpKind) -> Check {
        let compiler = self.cfg.compiler.clone();
        let got = self.dump_of(&compiler, spec, kind, false)?;
        let noun = kind.noun();
        if kind.count(&got) == 0 {
            return Err(Problem::Fail(format!(
                "the compiler dumped no {noun} (the test pins nothing)\n{}",
                indent(&got)
            )));
        }
        if let Some(oracle) = self.cfg.oracle.clone()
            && (self.cfg.compiler_kind == CompilerKind::Kiln || oracle != self.cfg.compiler)
        {
            let theirs = self.dump_of(&oracle, spec, kind, true)?;
            if kind.count(&theirs) == 0 {
                let hint = match kind {
                    DumpKind::Layout | DumpKind::Ir => "",
                    DumpKind::Vtables => {
                        "; clang dumps a vtable only when it emits it: give every dynamic \
                         class a key function defined in the test"
                    }
                };
                return Err(Problem::Broken(format!(
                    "the oracle ({}) dumped no {noun}{hint}",
                    oracle.display()
                )));
            }
            if let Some(d) = kind.diff(&theirs, &got) {
                return Err(Problem::Fail(d));
            }
        }
        Ok(())
    }

    /// Runs `program` with the dump flags and returns its standard output.
    /// kiln dumps under `-fsyntax-only`. clang dumps record layouts under
    /// `-fsyntax-only -Xclang -fdump-record-layouts-complete`, but vtables
    /// only while it emits them, so its vtable dump compiles to LLVM IR in
    /// the scratch directory. As the compiler under test the exit status and
    /// `diag:` lines are checked; as the oracle any failure is an ERROR.
    fn dump_of(
        &mut self,
        program: &Path,
        spec: &TestSpec,
        kind: DumpKind,
        oracle: bool,
    ) -> Result<String, Problem> {
        let kiln = !oracle && self.cfg.compiler_kind == CompilerKind::Kiln;
        let mode: &[&str] = match (kiln, kind) {
            (true, DumpKind::Layout) => &["-fsyntax-only", "-fdump-record-layouts"],
            (true, DumpKind::Vtables) => &["-fsyntax-only", "-fdump-vtable-layouts"],
            (false, DumpKind::Layout) => {
                &["-fsyntax-only", "-Xclang", "-fdump-record-layouts-complete"]
            }
            (false, DumpKind::Vtables) => &["-S", "-emit-llvm", "-Xclang", "-fdump-vtable-layouts"],
            (_, DumpKind::Ir) => &["-S", "-emit-llvm", "-O0"],
        };
        let target = ir_target_flag(kind, spec, std::env::consts::ARCH);
        let mode: Vec<&str> = mode.iter().copied().chain(target.as_deref()).collect();
        let mode = mode.as_slice();
        let ir = match (kiln, kind) {
            (false, DumpKind::Vtables) | (_, DumpKind::Ir) => {
                Some(self.scratch()?.join(if oracle { "oracle.ll" } else { "test.ll" }))
            }
            _ => None,
        };
        // `expect: ir` compares the module written to the scratch file.
        let module = |ir: Option<&Path>| -> Result<String, Problem> {
            let p = ir.unwrap_or(Path::new(""));
            fs::read_to_string(p)
                .map_err(|e| Problem::Broken(format!("cannot read {}: {e}", p.display())))
        };
        if oracle {
            let o = self.oracle_compile(program, spec, mode, ir.as_deref())?;
            return if kind == DumpKind::Ir { module(ir.as_deref()) } else { Ok(o.stdout_str()) };
        }
        let o = self.compile(spec, mode, ir.as_deref())?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "expected the {} dump, but the compiler failed ({})\n{}",
                kind.noun(),
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))?;
        if kind == DumpKind::Ir { module(ir.as_deref()) } else { Ok(o.stdout_str()) }
    }
}

/// The `--target=` an `expect: ir` test without one is compiled with, by
/// both the compiler and the oracle: the vendor of clang's default triple
/// is a build setting (`x86_64-pc-linux-gnu` for Debian's x86-64 clang,
/// `x86_64-unknown-linux-gnu` for kiln), not part of the program, and the
/// `target triple` line is compared exactly (`spec/harness.md` "IR tests").
fn ir_target_flag(kind: DumpKind, spec: &TestSpec, host: &str) -> Option<String> {
    (kind == DumpKind::Ir && spec.target_triple().is_none())
        .then(|| format!("--target={host}-unknown-linux-gnu"))
}

/// The debugger binary for `gdb`/`lldb` scripts: `gdb`, else `lldb-19`
/// then `lldb`, on `PATH`.
fn debugger_program(name: &str) -> Option<PathBuf> {
    debugger_program_in(name, std::env::var_os("PATH").as_deref())
}

/// [`debugger_program`] with `path` as the value of `PATH`.
fn debugger_program_in(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let names: &[&str] = if name == "gdb" { &["gdb"] } else { &["lldb-19", "lldb"] };
    names.iter().find_map(|n| std::env::split_paths(path?).map(|d| d.join(n)).find(|p| p.is_file()))
}

/// The least time a debugger script gets, whatever `--timeout` says: a
/// debugger loads the binary's debug information and (lldb) its Python
/// runtime before the first command, which takes seconds on a busy CI
/// runner. A hung session still ends, and fails, deterministically.
const DEBUGGER_TIMEOUT: Duration = Duration::from_secs(60);

/// A `debugger` test that drives a debugger not on `PATH` cannot run: a
/// SKIP, or under `require` (`KILN_REQUIRE_DEBUGGERS=1`) an ERROR, with
/// the reason.
fn debugger_gate(spec: &TestSpec, require: bool) -> Option<(Status, String)> {
    debugger_gate_in(spec, std::env::var_os("PATH").as_deref(), require)
}

/// [`debugger_gate`] with `path` as the value of `PATH`.
fn debugger_gate_in(
    spec: &TestSpec,
    path: Option<&std::ffi::OsStr>,
    require: bool,
) -> Option<(Status, String)> {
    if spec.expect != Expect::Debugger {
        return None;
    }
    let (name, _) = [("gdb", &spec.gdb), ("lldb", &spec.lldb)]
        .into_iter()
        .find(|(name, sc)| !sc.commands.is_empty() && debugger_program_in(name, path).is_none())?;
    Some(if require {
        let env = super::REQUIRE_DEBUGGERS_ENV;
        (Status::Error, format!("{env}=1 and `{name}` is not on PATH (`expect: debugger`)"))
    } else {
        (Status::Skip, format!("requires `{name}` on PATH (`expect: debugger`)"))
    })
}

/// The first of `checks` that no output line after the previous match
/// contains (checks match successive lines, in order).
fn first_missing_check<'c>(out: &str, checks: &'c [String]) -> Option<&'c str> {
    let mut lines = out.lines();
    checks.iter().find(|c| !lines.by_ref().any(|l| l.contains(c.as_str()))).map(String::as_str)
}

/// An LLVM module without what differs between producers but not in the
/// program: comment lines and trailing comments (`; preds = ...`), the
/// `; ModuleID` and `source_filename` lines, and `!llvm.ident` with its
/// `"... version ..."` string node (`spec/harness.md` "IR tests").
fn ir_normalize(ir: &str) -> String {
    let mut out = String::new();
    for line in ir.lines() {
        let line = match line.find("; preds = ") {
            Some(i) if !line.starts_with(';') => line[..i].trim_end(),
            _ => line,
        };
        let producer = line.starts_with('!')
            && line.contains("= !{!\"")
            && (line.contains("clang version") || line.contains("kiln version"));
        if line.starts_with(';')
            || line.starts_with("source_filename")
            || line.starts_with("!llvm.ident")
            || producer
        {
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Compares two LLVM modules after [`ir_normalize`]: a line diff, or
/// `None` when they agree.
fn ir_diff(want: &str, got: &str) -> Option<String> {
    let (theirs, ours) = (ir_normalize(want), ir_normalize(got));
    (theirs != ours).then(|| {
        format!(
            "the LLVM IR differs from the oracle's\n--- diff (-oracle +actual) ---\n{}",
            indent(&line_diff(&theirs, &ours))
        )
    })
}

/// Compares record-layout dumps block by block, in order (clang's builtin
/// records dropped): the first differing block with both texts and a line
/// diff, or `None` when they agree.
fn layout_diff(want: &str, got: &str) -> Option<String> {
    let (theirs, ours) = (record_layout_blocks(want), record_layout_blocks(got));
    if theirs == ours {
        return None;
    }
    let first = theirs.iter().zip(&ours).take_while(|(t, o)| t == o).count();
    let oracle = theirs.get(first).copied().unwrap_or("");
    let actual = ours.get(first).copied().unwrap_or("");
    let shown =
        |blk: &str| if blk.is_empty() { "    <no block>\n".to_owned() } else { indent(blk) };
    Some(format!(
        "record layout block {} differs from the oracle's ({} blocks, oracle {}):\n\
         --- oracle ---\n{}--- actual ---\n{}--- diff (-oracle +actual) ---\n{}",
        first + 1,
        ours.len(),
        theirs.len(),
        shown(oracle),
        shown(actual),
        indent(&line_diff(oracle, actual))
    ))
}

/// The most blocks shown on each side of a vtable mismatch.
const MAX_SHOWN_BLOCKS: usize = 4;

/// Compares vtable dumps as sets of blocks (`Original map` sections and
/// repeats dropped). Blocks with the same first line on both sides are
/// shown as a line diff; the rest are listed per side. `None` when the
/// sets are equal.
fn vtables_diff(want: &str, got: &str) -> Option<String> {
    let (w, g) = (vtable_blocks(want), vtable_blocks(got));
    if w == g {
        return None;
    }
    let head = |b: &str| b.lines().next().unwrap_or_default().to_owned();
    let only_w: Vec<&String> = w.difference(&g).collect();
    let only_g: Vec<&String> = g.difference(&w).collect();
    let mut out = format!(
        "vtable dump differs from the oracle's, as sets of blocks ({} blocks, oracle {}):\n",
        g.len(),
        w.len()
    );
    let mut paired = Vec::new();
    let mut shown = 0;
    for a in &only_w {
        if let Some(b) = only_g.iter().find(|b| head(b) == head(a)) {
            paired.push(head(a));
            if shown < MAX_SHOWN_BLOCKS {
                let _ = write!(
                    out,
                    "block `{}` differs (-oracle +actual):\n{}",
                    head(a),
                    indent(&line_diff(a, b))
                );
            }
            shown += 1;
        }
    }
    for (side, blocks) in [("the oracle's", &only_w), ("the actual", &only_g)] {
        let rest: Vec<&&String> = blocks.iter().filter(|b| !paired.contains(&head(b))).collect();
        if rest.is_empty() {
            continue;
        }
        let _ = writeln!(out, "only in {side} dump ({}):", rest.len());
        for b in rest.iter().take(MAX_SHOWN_BLOCKS) {
            out.push_str(&indent(b));
        }
        if rest.len() > MAX_SHOWN_BLOCKS {
            let _ = writeln!(out, "    ... and {} more", rest.len() - MAX_SHOWN_BLOCKS);
        }
    }
    if shown > MAX_SHOWN_BLOCKS {
        let _ = writeln!(out, "... and {} more differing blocks", shown - MAX_SHOWN_BLOCKS);
    }
    Some(out)
}

/// A line diff of two dumps, or `None` when they are equal up to trailing
/// whitespace on each line and trailing blank lines.
fn ast_diff(want: &str, got: &str) -> Option<String> {
    let norm = |s: &str| {
        let mut lines: Vec<&str> = s.lines().map(str::trim_end).collect();
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    };
    let (w, g) = (norm(want), norm(got));
    (w != g).then(|| line_diff(&w, &g))
}

/// Indents every line by four spaces (and terminates the last one).
/// The last `n` lines of `s` (an out-of-memory death's stderr can be long;
/// the cause is at its end).
fn tail(s: &str, n: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    let skip = lines.len().saturating_sub(n);
    let mut out = if skip > 0 { format!("[{skip} earlier lines]\n") } else { String::new() };
    for l in &lines[skip..] {
        out.push_str(l);
        out.push('\n');
    }
    out
}

pub fn indent(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The `-fclang-compat` level for a kiln run beside a clang `oracle`
/// major (`Runner::clang_compat`).
fn compat_for(
    oracle: u32,
    at_least: Option<directives::ClangVersion>,
    below: Option<directives::ClangVersion>,
) -> u32 {
    let levels = crate::oracle::SUPPORTED;
    if let Some(r) = at_least.filter(|r| oracle < r[0]) {
        return levels.iter().copied().find(|&l| l >= r[0]).unwrap_or(oracle);
    }
    if let Some(b) = below.filter(|b| oracle >= b[0]) {
        return levels.iter().copied().rev().find(|&l| l < b[0]).unwrap_or(oracle);
    }
    oracle
}

/// Helpers of `tests/support/kiln_meta.h` that its header-differential
/// stand-in (`tests/support/meta-header/kiln_meta.h`) also defines, over
/// the standard API: tests that use them still run in the mode.
const META_HEADER_HELPERS: &[&str] = &["__kiln_data_members", "__kiln_info_array"];

/// Functions `tests/support/kiln_meta.h` declares in `std::meta` that the
/// standard does not (kiln extensions without the `__kiln_` prefix).
const META_HEADER_EXTENSIONS: &[&str] = &["name_of"];

/// Why the header-differential mode (`--meta-header`) leaves a test out,
/// or `None` when it runs it: it must include a `kiln_meta*.h`, not say
/// `meta-header: skip`, be a kind whose outcome does not depend on the
/// header's declarations (`compile-ok`, `compile-fail`, `run`,
/// `no-crash`), not select libc++, and name no kiln extension
/// (`__kiln_*`, other than [`META_HEADER_HELPERS`], or a call of one of
/// [`META_HEADER_EXTENSIONS`]).
pub(super) fn meta_header_skip(spec: &TestSpec, source: &str) -> Option<String> {
    const MODE: &str = "meta-header mode";
    let includes = source.lines().any(|l| {
        let t = l.trim_start();
        t.starts_with('#') && t.contains("include") && t.contains("\"kiln_meta")
    });
    if !includes {
        return Some(format!("{MODE}: does not include kiln_meta.h"));
    }
    if let Some(r) = &spec.meta_header_skip {
        return Some(format!("{MODE}: {r}"));
    }
    if !matches!(
        spec.expect,
        Expect::CompileOk | Expect::CompileFail | Expect::Run | Expect::NoCrash
    ) {
        return Some(format!(
            "{MODE}: a {:?} test compares kiln_meta.h's own declarations",
            spec.expect
        ));
    }
    if spec.args.iter().any(|a| a == "-stdlib=libc++" || a.starts_with("-nostdinc")) {
        return Some(format!("{MODE}: the test does not use libstdc++"));
    }
    let mut rest = source;
    while let Some(i) = rest.find("__kiln_") {
        let tail = &rest[i..];
        let end =
            tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(tail.len());
        let ident = &tail[..end];
        if !META_HEADER_HELPERS.contains(&ident) {
            return Some(format!("{MODE}: uses the kiln extension `{ident}`"));
        }
        rest = &tail[end..];
    }
    for ext in META_HEADER_EXTENSIONS {
        let call = format!("{ext}(");
        let called = source
            .match_indices(&call)
            .any(|(i, _)| !source[..i].ends_with(|c: char| c.is_ascii_alphanumeric() || c == '_'));
        if called {
            return Some(format!("{MODE}: uses the kiln extension `{ext}`"));
        }
    }
    None
}

#[cfg(test)]
mod compat_tests {
    use super::{
        DumpKind, Status, compat_for, debugger_gate_in, first_missing_check, ir_diff, ir_normalize,
        ir_target_flag,
    };
    use crate::test::directives::{Lang, parse};

    /// spec/harness.md "Oracle toolchain": kiln imitates the oracle, or
    /// the clang a `requires:` names when the oracle does not meet it.
    #[test]
    fn compat_level() {
        assert_eq!(compat_for(22, None, None), 22);
        assert_eq!(compat_for(19, None, None), 19);
        assert_eq!(compat_for(19, Some([22, 0, 0]), None), 22);
        assert_eq!(compat_for(22, Some([22, 0, 0]), None), 22);
        assert_eq!(compat_for(19, Some([20, 0, 0]), None), 22);
        assert_eq!(compat_for(22, None, Some([22, 0, 0])), 19);
        assert_eq!(compat_for(19, None, Some([22, 0, 0])), 19);
    }

    #[test]
    fn debugger_checks_match_successive_lines_in_order() {
        let out = "Breakpoint 1, f (v=3) at t.cpp:4\n$1 = 3\n$2 = {x = 1, y = 2}\n";
        let c = |v: &[&str]| v.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
        assert_eq!(first_missing_check(out, &c(&["t.cpp:4", "$1 = 3", "x = 1"])), None);
        // In order: `$1` cannot match before the breakpoint line's match.
        assert_eq!(first_missing_check(out, &c(&["$2", "$1 = 3"])), Some("$1 = 3"));
        // One line satisfies one check only.
        assert_eq!(first_missing_check(out, &c(&["$1", "= 3"])), Some("= 3"));
        assert_eq!(first_missing_check(out, &c(&["nowhere"])), Some("nowhere"));
    }

    /// spec/harness.md "Debugger tests": a missing debugger is a SKIP,
    /// an ERROR under `KILN_REQUIRE_DEBUGGERS=1`, and nothing when the
    /// test does not drive it.
    #[test]
    fn debugger_gate_skips_or_errors_without_the_debugger() {
        let dir = std::env::temp_dir().join(format!("kiln-debugger-gate-{}", std::process::id()));
        let (empty, tools) = (dir.join("empty"), dir.join("tools"));
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("gdb"), "").unwrap();
        let both = parse(
            "//@ expect: debugger\n//@ gdb: run\n//@ gdb-check: x\n//@ lldb: run\n\
             //@ lldb-check: x\n",
            Lang::Cxx,
        )
        .unwrap();
        let gdb_only =
            parse("//@ expect: debugger\n//@ gdb: run\n//@ gdb-check: x\n", Lang::Cxx).unwrap();
        let none = empty.as_os_str();
        let (status, why) = debugger_gate_in(&both, Some(none), false).unwrap();
        assert_eq!(status, Status::Skip);
        assert_eq!(why, "requires `gdb` on PATH (`expect: debugger`)");
        let (status, why) = debugger_gate_in(&both, Some(none), true).unwrap();
        assert_eq!(status, Status::Error);
        assert_eq!(why, "KILN_REQUIRE_DEBUGGERS=1 and `gdb` is not on PATH (`expect: debugger`)");
        assert_eq!(debugger_gate_in(&both, None, true).unwrap().0, Status::Error);
        // gdb found, lldb missing.
        let (status, why) = debugger_gate_in(&both, Some(tools.as_os_str()), true).unwrap();
        assert_eq!(status, Status::Error);
        assert!(why.contains("`lldb`"), "{why}");
        // A test that drives only gdb needs only gdb.
        assert_eq!(debugger_gate_in(&gdb_only, Some(tools.as_os_str()), true), None);
        std::fs::write(tools.join("lldb-19"), "").unwrap();
        assert_eq!(debugger_gate_in(&both, Some(tools.as_os_str()), true), None);
        // Not a debugger test: never gated.
        let plain = parse("//@ expect: compile-ok\n", Lang::Cxx).unwrap();
        assert_eq!(debugger_gate_in(&plain, Some(none), true), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn ir_normalize_drops_producer_lines_and_comments() {
        let clang = "; ModuleID = 'a.cpp'\nsource_filename = \"a.cpp\"\n\n\
                     ; Function Attrs: noinline\ndefine void @f() #0 {\n  br label %1\n\n\
                     1:                                                ; preds = %0\n  ret void\n}\n\
                     !llvm.ident = !{!0}\n!0 = !{!\"Debian clang version 22.1.8\"}\n";
        let kiln = "; ModuleID = 'a.cpp'\nsource_filename = \"b.cpp\"\n\n\
                    define void @f() #0 {\n  br label %1\n\n1:\n  ret void\n}\n\
                    !llvm.ident = !{!0}\n!0 = !{!\"kiln version 0.0.1\"}\n";
        assert_eq!(ir_normalize(clang), ir_normalize(kiln));
        assert!(ir_diff(clang, kiln).is_none());
        let d = ir_diff(clang, &kiln.replace("ret void", "unreachable")).expect("a diff");
        assert!(d.contains("+  unreachable") && d.contains("-  ret void"), "{d}");
    }

    /// spec/harness.md "IR tests": both compilers are given the triple
    /// (clang's default vendor differs by build), unless the test names one.
    #[test]
    fn ir_tests_name_the_target_triple() {
        let plain = parse("//@ expect: ir\n", Lang::Cxx).unwrap();
        let flag = ir_target_flag(DumpKind::Ir, &plain, "x86_64");
        assert_eq!(flag.as_deref(), Some("--target=x86_64-unknown-linux-gnu"));
        let own =
            parse("//@ expect: ir\n//@ args: --target=aarch64-linux-gnu\n", Lang::Cxx).unwrap();
        assert_eq!(ir_target_flag(DumpKind::Ir, &own, "x86_64"), None);
        let layout = parse("//@ expect: layout\n", Lang::Cxx).unwrap();
        assert_eq!(ir_target_flag(DumpKind::Layout, &layout, "x86_64"), None);
    }
}
