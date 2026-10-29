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
    /// `--pp-compare`: overrides every test's `pp-compare:`.
    pub pp_compare: Option<PpCompare>,
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
        inst_report: None,
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
    /// The last compile's masked instantiation report (`inst-report:`).
    inst_report: Option<String>,
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
        if spec.is_kiln_specific() && kind != CompilerKind::Kiln {
            let directive = if spec.oven.is_empty() { "inst-report:" } else { "oven:" };
            let reason =
                format!("not supported by this compiler (kiln-specific `{directive}` directive)");
            return (Status::Skip, Some(reason), String::new());
        }
        if spec.expect == Expect::Ast && kind == CompilerKind::Other {
            let reason = "not supported by this compiler (`expect: ast` needs kiln or clang)";
            return (Status::Skip, Some(reason.to_owned()), String::new());
        }
        let outcome = match spec.expect {
            Expect::CompileOk => self.compile_ok(&spec),
            Expect::CompileFail => self.compile_fail(&spec),
            Expect::Run => self.run(&spec),
            Expect::Pp => self.pp(&spec),
            Expect::Ast => self.ast(&spec),
        };
        // Budgets are checked once the kind's own checks pass.
        let outcome = outcome.and_then(|()| self.check_oven(&spec));
        let outcome = outcome.and_then(|()| self.check_inst_report(&spec));
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
        c.args(spec.args.iter().map(|a| directives::substitute(a, |n| self.placeholder(n))));
        c.arg(&self.case.name);
        if let Some(o) = out {
            c.arg("-o").arg(o);
        }
        c
    }

    /// The value of an `args:` placeholder (`spec/harness.md` "Placeholders").
    fn placeholder(&self, name: &str) -> Option<String> {
        let path = match name {
            "repo_root" => self.cfg.root.clone(),
            "resource_dir" => self.cfg.root.join("resource"),
            "test_dir" => self.case.path.parent()?.to_path_buf(),
            _ => return None,
        };
        Some(path.to_string_lossy().into_owned())
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
        let o = self.exec(cmd, self.cfg.compile_timeout).map_err(Problem::Broken)?;
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
        let cmd = self.command(oracle, spec, mode, out);
        let o = self.exec(cmd, self.cfg.compile_timeout).map_err(Problem::Broken)?;
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

    fn report_path(&self) -> Result<PathBuf, Problem> {
        Ok(self.scratch()?.join("constexpr-report.jsonl"))
    }

    /// Reads the constexpr report of the compile and checks the `oven:`
    /// budgets. A missing or malformed report is kiln's fault: FAIL.
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
    /// arguments, environment and stdin.
    fn execute_binary(
        &mut self,
        exe: &Path,
        spec: &TestSpec,
        stdin: Option<&[u8]>,
    ) -> Result<Output, String> {
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
        let r = process::run_with_input(&mut cmd, stdin, self.cfg.run_timeout);
        match &r {
            Ok(o) => self.log.push(format!("$ {shown}  # {}", o.describe_end())),
            Err(e) => self.log.push(format!("$ {shown}  # failed to start: {e}")),
        }
        r.map_err(|e| format!("could not run `{shown}`: {e}"))
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
        let stdin = self.stdin_bytes(spec)?;
        let got = self.execute_binary(&exe, spec, stdin.as_deref()).map_err(Problem::Fail)?;
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
            let theirs =
                self.execute_binary(&oexe, spec, stdin.as_deref()).map_err(Problem::Broken)?;
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
            let mut mode = vec!["-fsyntax-only", "-fdump-ast"];
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
        let opts = DumpOptions { filter: spec.ast_filter.clone(), ..DumpOptions::default() };
        let main_file = self.case.name.clone();
        let timeout = self.cfg.compile_timeout;
        let mut text = || {
            let cmd = self.command(clang, spec, &["-fsyntax-only", "-Xclang", "-ast-dump"], None);
            let o = self.exec(cmd, timeout)?;
            if o.success() {
                Ok(o.stdout_str())
            } else {
                Err(format!("clang's text dump failed ({})", o.describe_end()))
            }
        };
        normalize(&json, &main_file, &opts, &mut text).map_err(|e| {
            Problem::Broken(format!(
                "cannot normalize the AST dump of {} (`xtask clang-ast`):\n{}",
                clang.display(),
                indent(&e)
            ))
        })
    }
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
pub fn indent(s: &str) -> String {
    let mut out = String::new();
    for line in s.lines() {
        out.push_str("    ");
        out.push_str(line);
        out.push('\n');
    }
    out
}
