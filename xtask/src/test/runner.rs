//! Executing one test (`spec/harness.md` "Test kinds").

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use super::compare::{line_diff, normalize_pp, pp_equivalent};
use super::diag_parse::{self, Diagnostic};
use super::directives::{self, CompilerKind, Expect, Lang, Severity, StdoutExpect, TestSpec};
use super::discover::TestCase;
use crate::util::process::{self, Output, Shown};

/// Everything a worker needs; shared read-only between threads.
#[derive(Debug)]
pub struct Config {
    /// Repository root; compilers run with this as their working directory.
    pub root: PathBuf,
    pub compiler: PathBuf,
    pub compiler_kind: CompilerKind,
    /// The oracle compiler; `None` disables oracle comparisons (`--no-oracle`).
    pub oracle: Option<PathBuf>,
    pub update: bool,
    /// Per-run scratch directory (under the cargo target dir).
    pub scratch: PathBuf,
    pub compile_timeout: Duration,
    pub run_timeout: Duration,
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
        }
    }

    /// Does this outcome make the run fail?
    pub fn is_failure(self) -> bool {
        matches!(self, Self::Fail | Self::XPass | Self::Error)
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
    pub elapsed: Duration,
}

enum Problem {
    Fail(String),
    Broken(String),
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
        elapsed: start.elapsed(),
    }
}

struct Test<'a> {
    cfg: &'a Config,
    case: &'a TestCase,
    dir: PathBuf,
    log: Vec<String>,
    updated: bool,
}

impl Test<'_> {
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
        let kind = self.cfg.compiler_kind;
        if let Some(r) = spec.skip_reason(kind) {
            return (Status::Skip, Some(r.to_owned()), String::new());
        }
        if spec.is_kiln_specific() {
            let reason = if kind == CompilerKind::Kiln {
                // TODO(oven): read the Oven's step/byte counters (e.g. via
                // `-fconstexpr-report`) and enforce the budget.
                "oven: budget checking is not implemented in the harness yet"
            } else {
                "not supported by this compiler (kiln-specific `oven:` directive)"
            };
            return (Status::Skip, Some(reason.to_owned()), String::new());
        }
        let outcome = match spec.expect {
            Expect::CompileOk => self.compile_ok(&spec),
            Expect::CompileFail => self.compile_fail(&spec),
            Expect::Run => self.run(&spec),
            Expect::Pp => self.pp(&spec),
        };
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
        }
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
        c.args(&spec.args);
        c.arg(&self.case.name);
        if let Some(o) = out {
            c.arg("-o").arg(o);
        }
        c
    }

    fn exec(&mut self, mut cmd: Command, timeout: Duration) -> Result<Output, String> {
        let shown = Shown(&cmd).to_string();
        let r = process::run(&mut cmd, timeout);
        match &r {
            Ok(o) => self.log.push(format!("$ {shown}  # {}", o.describe_end())),
            Err(e) => self.log.push(format!("$ {shown}  # failed to start: {e}")),
        }
        r.map_err(|e| format!("could not run `{shown}`: {e}"))
    }

    /// Runs the compiler under test and rejects timeouts and crashes.
    fn compile(
        &mut self,
        spec: &TestSpec,
        mode: &[&str],
        out: Option<&Path>,
    ) -> Result<Output, Problem> {
        let cmd = self.command(&self.cfg.compiler, spec, mode, out);
        let o = self.exec(cmd, self.cfg.compile_timeout).map_err(Problem::Broken)?;
        let stderr = o.stderr_str();
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
        let cmd = self.command(oracle, spec, mode, out);
        let o = self.exec(cmd, self.cfg.compile_timeout).map_err(Problem::Broken)?;
        if o.success() {
            Ok(o)
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
        let report = diag_parse::match_diags(
            &spec.diags,
            diags,
            &|f| self.in_test_file(f),
            spec.strict_diags,
        );
        if report.is_ok() {
            Ok(())
        } else {
            Err(Problem::Fail(diag_parse::explain(&report, &spec.diags, diags)))
        }
    }

    fn compile_ok(&mut self, spec: &TestSpec) -> Check {
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

    fn scratch(&self) -> Result<&Path, Problem> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| Problem::Broken(format!("cannot create {}: {e}", self.dir.display())))?;
        Ok(&self.dir)
    }

    fn execute_binary(&mut self, exe: &Path) -> Result<Output, String> {
        let mut cmd = Command::new(exe);
        cmd.current_dir(&self.dir);
        self.exec(cmd, self.cfg.run_timeout)
    }

    fn run(&mut self, spec: &TestSpec) -> Check {
        let exe = self.scratch()?.join("test.out");
        let o = self.compile(spec, &[], Some(&exe))?;
        if !o.success() {
            return Err(Problem::Fail(format!(
                "compiling the test failed ({})\n{}",
                o.describe_end(),
                indent(&o.stderr_str())
            )));
        }
        self.check_diags(spec, &diag_parse::parse(&o.stderr_str()))?;
        let got = self.execute_binary(&exe).map_err(Problem::Fail)?;
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
            self.oracle_compile(&oracle, spec, &[], Some(&oexe))?;
            let theirs = self.execute_binary(&oexe).map_err(Problem::Broken)?;
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
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }

    fn pp(&mut self, spec: &TestSpec) -> Check {
        let o = self.compile(spec, &["-E", "-P"], None)?;
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
            if self.cfg.update {
                if want != got {
                    fs::write(&golden, &got).map_err(|e| {
                        Problem::Broken(format!("cannot write {}: {e}", golden.display()))
                    })?;
                    self.updated = true;
                }
            } else if !pp_equivalent(&want, &got) {
                let _ = write!(
                    failures,
                    "output differs from {} after whitespace normalization (-expected +actual):\n{}",
                    golden.file_name().unwrap_or_default().to_string_lossy(),
                    indent(&line_diff(&normalize_pp(&want), &normalize_pp(&got)))
                );
            }
        }
        if spec.oracle
            && let Some(oracle) = self.cfg.oracle.clone()
        {
            let theirs = self.oracle_compile(&oracle, spec, &["-E", "-P"], None)?.stdout_str();
            if !pp_equivalent(&theirs, &got) {
                let _ = write!(
                    failures,
                    "output differs from the oracle after whitespace normalization (-oracle +actual):\n{}",
                    indent(&line_diff(&normalize_pp(&theirs), &normalize_pp(&got)))
                );
            }
        }
        if failures.is_empty() { Ok(()) } else { Err(Problem::Fail(failures)) }
    }
}

/// Indents every line by four spaces (and terminates the last one).
pub fn indent(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out
}
