//! The `//@` directive header of a conformance test (`spec/harness.md`,
//! `docs/VALIDATION.md` "Conformance test format").
//!
//! The header is the leading run of lines that are blank or `//` comments.
//! Every line in it whose trimmed text starts with `//@` is a directive of the
//! form `key: value` or (for flags) a bare `key`. A `//@` line after the header
//! is an error, so a misplaced directive can never be silently ignored.

use std::fmt;
use std::path::PathBuf;

pub use crate::util::CompilerKind;
pub use crate::util::header::{
    self, C_STDS, CXX_STDS, DirectiveError, OvenBudget, parse_oven, split_args,
};

/// What a test expects of the compiler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expect {
    /// `-fsyntax-only` succeeds.
    CompileOk,
    /// `-fsyntax-only` fails with at least one error in the test file.
    CompileFail,
    /// Compile, link and run; compare stdout and exit code.
    Run,
    /// `-E -P`; compare the preprocessed output.
    Pp,
}

impl Expect {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "compile-ok" => Self::CompileOk,
            "compile-fail" => Self::CompileFail,
            "run" => Self::Run,
            "pp" => Self::Pp,
            _ => return None,
        })
    }

    #[cfg(test)]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CompileOk => "compile-ok",
            Self::CompileFail => "compile-fail",
            Self::Run => "run",
            Self::Pp => "pp",
        }
    }
}

/// Source language, from the file extension (`.c` is C, everything else C++).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Cxx,
    C,
}

/// Diagnostic severity, shared by `diag:` directives and parsed compiler output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Fatal,
    Warning,
    Note,
    Remark,
}

impl Severity {
    /// Parses a directive severity word (`fatal` is the directive spelling of
    /// clang's `fatal error`).
    pub fn parse_directive(s: &str) -> Option<Self> {
        Some(match s {
            "error" => Self::Error,
            "fatal" => Self::Fatal,
            "warning" => Self::Warning,
            "note" => Self::Note,
            "remark" => Self::Remark,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Fatal => "fatal error",
            Self::Warning => "warning",
            Self::Note => "note",
            Self::Remark => "remark",
        }
    }

    /// Does a directive with severity `self` accept a diagnostic of `actual`?
    /// `error` also accepts `fatal error`; everything else is exact.
    pub fn accepts(self, actual: Severity) -> bool {
        self == actual || (self == Self::Error && actual == Self::Fatal)
    }
}

/// `diag: <line>[:<col>] <severity> "<substring>"`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagDirective {
    pub line: u32,
    /// `None` when the directive omits the column (any column matches).
    pub col: Option<u32>,
    pub severity: Severity,
    pub message: String,
    /// 1-based line of the directive itself, for error messages.
    pub directive_line: usize,
}

impl fmt::Display for DiagDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.col {
            Some(c) => write!(f, "{}:{}", self.line, c)?,
            None => write!(f, "{}", self.line)?,
        }
        write!(f, " {} {:?}", self.severity.as_str(), self.message)
    }
}

/// A `skip`/`xfail` marker: `skip: reason` (all compilers) or
/// `skip(kiln): reason` / `skip(clang): reason`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Marker {
    pub compiler: Option<CompilerKind>,
    pub reason: String,
}

/// Expected stdout of a `run` test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StdoutExpect {
    /// The `stdout:` lines joined with `\n`, each line `\n`-terminated.
    Inline(String),
    /// `stdout-file:`, relative to the directory containing the test.
    File(PathBuf),
}

/// Everything the directive header says about one test.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestSpec {
    pub expect: Expect,
    /// The `-std=` value, e.g. `c++26`.
    pub std: String,
    pub args: Vec<String>,
    pub diags: Vec<DiagDirective>,
    pub strict_diags: bool,
    pub stdout: Option<StdoutExpect>,
    pub exit: Option<i32>,
    /// Compare against the oracle compiler (default: on for `run`/`pp`).
    pub oracle: bool,
    pub oven: Option<OvenBudget>,
    pub skip: Vec<Marker>,
    pub xfail: Vec<Marker>,
}

impl TestSpec {
    /// The skip reason that applies to `kind`, if any.
    pub fn skip_reason(&self, kind: CompilerKind) -> Option<&str> {
        marker_for(&self.skip, kind)
    }

    /// The xfail reason that applies to `kind`, if any.
    pub fn xfail_reason(&self, kind: CompilerKind) -> Option<&str> {
        marker_for(&self.xfail, kind)
    }

    /// Does this test use directives only kiln can check?
    pub fn is_kiln_specific(&self) -> bool {
        self.oven.is_some()
    }
}

fn marker_for(markers: &[Marker], kind: CompilerKind) -> Option<&str> {
    // A compiler-specific marker wins over a general one.
    markers
        .iter()
        .find(|m| m.compiler == Some(kind))
        .or_else(|| markers.iter().find(|m| m.compiler.is_none()))
        .map(|m| m.reason.as_str())
}

/// Parses the directive header of `source`. All errors are collected.
pub fn parse(source: &str, lang: Lang) -> Result<TestSpec, Vec<DirectiveError>> {
    let (directives, errors) = header::scan(source);
    let mut p = Parser::new(lang);
    p.errors = errors;
    for d in directives {
        p.directive(d.line, d.key, d.value);
    }
    p.finish()
}

struct Parser {
    lang: Lang,
    errors: Vec<DirectiveError>,
    expect: Option<Expect>,
    std: Option<String>,
    args: Vec<String>,
    diags: Vec<DiagDirective>,
    strict_diags: bool,
    stdout_lines: Option<String>,
    stdout_file: Option<PathBuf>,
    exit: Option<i32>,
    oracle: Option<bool>,
    oven: Option<OvenBudget>,
    skip: Vec<Marker>,
    xfail: Vec<Marker>,
    /// Lines of directives whose validity depends on `expect`.
    run_only: Vec<(usize, &'static str)>,
    oracle_line: usize,
    stdout_file_line: usize,
}

impl Parser {
    fn new(lang: Lang) -> Self {
        Self {
            lang,
            errors: Vec::new(),
            expect: None,
            std: None,
            args: Vec::new(),
            diags: Vec::new(),
            strict_diags: false,
            stdout_lines: None,
            stdout_file: None,
            exit: None,
            oracle: None,
            oven: None,
            skip: Vec::new(),
            xfail: Vec::new(),
            run_only: Vec::new(),
            oracle_line: 0,
            stdout_file_line: 0,
        }
    }

    fn err(&mut self, line: usize, message: impl Into<String>) {
        self.errors.push(DirectiveError { line, message: message.into() });
    }

    fn directive(&mut self, line: usize, key: &str, value: Option<&str>) {
        let Some((name, qualifier, value)) = self.split_key(line, key, value) else {
            return;
        };
        match name {
            "expect" => match Expect::parse(value) {
                Some(e) if self.expect.is_none() => self.expect = Some(e),
                Some(_) => self.err(line, "duplicate `expect`"),
                None => self.err(
                    line,
                    format!("unknown expectation `{value}` (compile-ok|compile-fail|run|pp)"),
                ),
            },
            "std" => self.std_directive(line, value),
            "args" => match split_args(value) {
                Ok(mut a) => self.args.append(&mut a),
                Err(e) => self.err(line, e),
            },
            "diag" => match parse_diag(value, line) {
                Ok(d) => {
                    if d.severity == Severity::Note && self.diags.is_empty() {
                        self.err(
                            line,
                            "a `note` diag must follow the error/warning/remark it belongs to",
                        );
                    }
                    self.diags.push(d);
                }
                Err(e) => self.err(line, e),
            },
            "stdout" => {
                self.run_only.push((line, "stdout"));
                let s = self.stdout_lines.get_or_insert_with(String::new);
                s.push_str(value);
                s.push('\n');
            }
            "stdout-file" => self.stdout_file_directive(line, value),
            "exit" => {
                self.run_only.push((line, "exit"));
                match value.parse::<i32>() {
                    Ok(c) if (0..=255).contains(&c) && self.exit.is_none() => self.exit = Some(c),
                    Ok(c) if (0..=255).contains(&c) => self.err(line, "duplicate `exit`"),
                    _ => self.err(line, format!("`exit` must be 0..=255, got `{value}`")),
                }
            }
            "oracle" => self.oracle_directive(line, value),
            "oven" => match parse_oven(value) {
                Ok(_) if self.oven.is_some() => self.err(line, "duplicate `oven`"),
                Ok(b) => self.oven = Some(b),
                Err(e) => self.err(line, e),
            },
            "skip" | "xfail" => self.marker(line, name, qualifier, value),
            _ => self.err(line, format!("unknown directive `{name}`")),
        }
    }

    /// Splits `name(qualifier)` and handles flag directives. Returns `None`
    /// when the directive was fully handled (or rejected) here.
    fn split_key<'s>(
        &mut self,
        line: usize,
        key: &'s str,
        value: Option<&'s str>,
    ) -> Option<(&'s str, Option<&'s str>, &'s str)> {
        // `skip(kiln)` / `xfail(clang)`: an optional compiler qualifier.
        let (name, qualifier) = match key.split_once('(') {
            Some((n, rest)) => {
                let Some(q) = rest.strip_suffix(')') else {
                    self.err(line, format!("malformed directive key `{key}`"));
                    return None;
                };
                (n.trim(), Some(q.trim()))
            }
            None => (key, None),
        };
        if qualifier.is_some() && name != "skip" && name != "xfail" {
            self.err(line, format!("`{name}` does not take a compiler qualifier"));
            return None;
        }
        if name == "strict-diags" {
            if value.is_some() {
                self.err(line, "`strict-diags` takes no value");
            } else if self.strict_diags {
                self.err(line, "duplicate `strict-diags`");
            }
            self.strict_diags = true;
            return None;
        }
        let Some(value) = value else {
            if is_known_key(name) {
                self.err(line, format!("`{name}` requires a value (`{name}: ...`)"));
            } else {
                self.err(line, format!("unknown directive `{name}`"));
            }
            return None;
        };
        Some((name, qualifier, value))
    }

    fn stdout_file_directive(&mut self, line: usize, value: &str) {
        self.run_only.push((line, "stdout-file"));
        if self.stdout_file.is_some() {
            self.err(line, "duplicate `stdout-file`");
        } else if value.is_empty() {
            self.err(line, "`stdout-file` requires a path");
        } else {
            self.stdout_file = Some(PathBuf::from(value));
            self.stdout_file_line = line;
        }
    }

    fn oracle_directive(&mut self, line: usize, value: &str) {
        let v = match value {
            "clang" => Some(true),
            "none" => Some(false),
            _ => None,
        };
        match v {
            Some(_) if self.oracle.is_some() => self.err(line, "duplicate `oracle`"),
            Some(b) => {
                self.oracle = Some(b);
                self.oracle_line = line;
            }
            None => self.err(line, format!("unknown oracle `{value}` (clang|none)")),
        }
    }

    fn std_directive(&mut self, line: usize, value: &str) {
        let allowed = match self.lang {
            Lang::Cxx => CXX_STDS,
            Lang::C => C_STDS,
        };
        if self.std.is_some() {
            self.err(line, "duplicate `std`");
        } else if allowed.contains(&value) {
            self.std = Some(value.to_owned());
        } else {
            self.err(line, format!("unknown std `{value}` (allowed: {})", allowed.join("|")));
        }
    }

    fn marker(&mut self, line: usize, name: &str, qualifier: Option<&str>, reason: &str) {
        let compiler = match qualifier {
            None => None,
            Some("kiln") => Some(CompilerKind::Kiln),
            Some("clang") => Some(CompilerKind::Clang),
            Some(q) => {
                self.err(line, format!("unknown compiler `{q}` (kiln|clang)"));
                return;
            }
        };
        if reason.is_empty() {
            self.err(line, format!("`{name}` requires a reason"));
            return;
        }
        let list = if name == "skip" { &mut self.skip } else { &mut self.xfail };
        if list.iter().any(|m| m.compiler == compiler) {
            self.err(line, format!("duplicate `{name}` for the same compiler"));
            return;
        }
        list.push(Marker { compiler, reason: reason.to_owned() });
    }

    fn finish(mut self) -> Result<TestSpec, Vec<DirectiveError>> {
        let expect = self.expect.unwrap_or(Expect::CompileOk);
        if expect != Expect::Run {
            for (line, key) in std::mem::take(&mut self.run_only) {
                self.err(line, format!("`{key}` only applies to `expect: run`"));
            }
        }
        if self.oracle.is_some() && !matches!(expect, Expect::Run | Expect::Pp) {
            self.err(self.oracle_line, "`oracle` only applies to `run` and `pp` tests");
        }
        let stdout = match (self.stdout_lines.take(), self.stdout_file.take()) {
            (Some(_), Some(_)) => {
                let line = self.stdout_file_line;
                self.err(line, "`stdout` and `stdout-file` are mutually exclusive");
                None
            }
            (Some(s), None) => Some(StdoutExpect::Inline(s)),
            (None, Some(p)) => Some(StdoutExpect::File(p)),
            (None, None) => None,
        };
        if !self.errors.is_empty() {
            return Err(self.errors);
        }
        let default_std = match self.lang {
            Lang::Cxx => "c++26",
            Lang::C => "c17",
        };
        Ok(TestSpec {
            expect,
            std: self.std.unwrap_or_else(|| default_std.to_owned()),
            args: self.args,
            diags: self.diags,
            strict_diags: self.strict_diags,
            stdout,
            exit: self.exit,
            oracle: self.oracle.unwrap_or(matches!(expect, Expect::Run | Expect::Pp)),
            oven: self.oven,
            skip: self.skip,
            xfail: self.xfail,
        })
    }
}

fn is_known_key(k: &str) -> bool {
    matches!(
        k,
        "expect"
            | "std"
            | "args"
            | "diag"
            | "stdout"
            | "stdout-file"
            | "exit"
            | "oracle"
            | "oven"
            | "skip"
            | "xfail"
    )
}

/// `<line>[:<col>] <severity> "<substring>"`.
fn parse_diag(value: &str, directive_line: usize) -> Result<DiagDirective, String> {
    let usage = "expected `diag: <line>[:<col>] <error|fatal|warning|note|remark> \"<substring>\"`";
    let (loc, rest) = value.split_once(char::is_whitespace).ok_or(usage)?;
    let (line, col) = match loc.split_once(':') {
        Some((l, c)) => (l, Some(c)),
        None => (loc, None),
    };
    let line: u32 = line
        .parse()
        .ok()
        .filter(|&l| l > 0)
        .ok_or_else(|| format!("bad line number in `{loc}`; {usage}"))?;
    let col = match col {
        Some(c) => Some(
            c.parse::<u32>()
                .ok()
                .filter(|&c| c > 0)
                .ok_or_else(|| format!("bad column in `{loc}`; {usage}"))?,
        ),
        None => None,
    };
    let rest = rest.trim_start();
    let (sev, rest) = rest.split_once(char::is_whitespace).ok_or(usage)?;
    let severity = Severity::parse_directive(sev)
        .ok_or_else(|| format!("unknown severity `{sev}`; {usage}"))?;
    let message = parse_quoted(rest.trim()).map_err(|e| format!("{e}; {usage}"))?;
    Ok(DiagDirective { line, col, severity, message, directive_line })
}

/// A `"..."` string with `\"` and `\\` escapes, and nothing after it.
fn parse_quoted(s: &str) -> Result<String, String> {
    let mut chars = s.chars();
    if chars.next() != Some('"') {
        return Err("message must be a double-quoted string".into());
    }
    let mut out = String::new();
    loop {
        match chars.next() {
            None => return Err("unterminated message string".into()),
            Some('"') => break,
            Some('\\') => match chars.next() {
                Some(c @ ('"' | '\\')) => out.push(c),
                Some(c) => return Err(format!("unknown escape `\\{c}` (only \\\" and \\\\)")),
                None => return Err("unterminated message string".into()),
            },
            Some(c) => out.push(c),
        }
    }
    if chars.as_str().trim().is_empty() {
        Ok(out)
    } else {
        Err(format!("unexpected text after message: `{}`", chars.as_str().trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(src: &str) -> TestSpec {
        parse(src, Lang::Cxx).unwrap_or_else(|e| panic!("unexpected errors: {e:?}"))
    }

    fn errs(src: &str) -> Vec<String> {
        parse(src, Lang::Cxx)
            .expect_err("expected directive errors")
            .into_iter()
            .map(|e| e.to_string())
            .collect()
    }

    #[test]
    fn defaults_without_header() {
        let s = ok("int main() {}\n");
        assert_eq!(s.expect, Expect::CompileOk);
        assert_eq!(s.std, "c++26");
        assert!(s.args.is_empty() && s.diags.is_empty() && !s.strict_diags);
        assert!(!s.oracle, "oracle is off by default for compile-ok");
        assert_eq!(s.exit, None);
        assert_eq!(parse("", Lang::C).unwrap().std, "c17");
    }

    #[test]
    fn validation_md_example() {
        let s = ok(concat!(
            "//@ std: c++26\n",
            "//@ expect: compile-fail\n",
            "//@ diag: 7:5 error \"static assertion failed\"\n",
            "//@ diag: 7:5 note \"evaluates to\"\n",
            "constexpr int f(int x) { return x * 2; }\n",
            "static_assert(f(2) == 5);\n",
        ));
        assert_eq!(s.expect, Expect::CompileFail);
        assert_eq!(s.diags.len(), 2);
        assert_eq!(
            s.diags[0],
            DiagDirective {
                line: 7,
                col: Some(5),
                severity: Severity::Error,
                message: "static assertion failed".into(),
                directive_line: 3,
            }
        );
        assert_eq!(s.diags[1].severity, Severity::Note);
    }

    #[test]
    fn run_directives() {
        let s = ok(concat!(
            "// a leading ordinary comment is part of the header\n",
            "\n",
            "//@ expect: run\n",
            "//@ std: c++20\n",
            "//@ args: -O2 -DNAME=\"a b\" 'x y'\n",
            "//@ args: -Wall\n",
            "//@ stdout: hello\n",
            "//@ stdout:\n",
            "//@ stdout: world  \n",
            "//@ exit: 3\n",
            "int main() {}\n",
        ));
        assert_eq!(s.expect, Expect::Run);
        assert_eq!(s.std, "c++20");
        assert_eq!(s.args, ["-O2", "-DNAME=a b", "x y", "-Wall"]);
        assert_eq!(s.stdout, Some(StdoutExpect::Inline("hello\n\nworld\n".into())));
        assert_eq!(s.exit, Some(3));
        assert!(s.oracle, "oracle defaults on for run");
    }

    #[test]
    fn stdout_file_and_oracle_none() {
        let s = ok("//@ expect: run\n//@ stdout-file: out.txt\n//@ oracle: none\n");
        assert_eq!(s.stdout, Some(StdoutExpect::File("out.txt".into())));
        assert!(!s.oracle);
        let s = ok("//@ expect: pp\n");
        assert!(s.oracle, "oracle defaults on for pp");
    }

    #[test]
    fn diag_forms() {
        let s = ok(concat!(
            "//@ diag: 3 warning \"no column\"\n",
            "//@ diag: 4:1 fatal \"esc \\\"q\\\" \\\\\"\n",
            "//@ diag: 5:2 remark \"\"\n",
            "//@ strict-diags\n",
        ));
        assert!(s.strict_diags);
        assert_eq!(s.diags[0].col, None);
        assert_eq!(s.diags[1].severity, Severity::Fatal);
        assert_eq!(s.diags[1].message, "esc \"q\" \\");
        assert_eq!(s.diags[2].message, "");
        assert_eq!(s.diags[0].to_string(), "3 warning \"no column\"");
    }

    #[test]
    fn markers_and_qualifiers() {
        let s = ok(concat!(
            "//@ xfail(clang): clang 19 lacks P2996\n",
            "//@ skip(kiln): not yet\n",
            "//@ xfail: everyone\n",
        ));
        assert_eq!(s.xfail_reason(CompilerKind::Clang), Some("clang 19 lacks P2996"));
        assert_eq!(s.xfail_reason(CompilerKind::Kiln), Some("everyone"));
        assert_eq!(s.skip_reason(CompilerKind::Kiln), Some("not yet"));
        assert_eq!(s.skip_reason(CompilerKind::Clang), None);
    }

    #[test]
    fn oven_budget() {
        let s = ok("//@ oven: bytes<=4096 steps<=100\n");
        assert_eq!(s.oven, Some(OvenBudget { steps: Some(100), bytes: Some(4096) }));
        assert!(s.is_kiln_specific());
        assert!(errs("//@ oven: steps<100\n")[0].contains("bad budget"));
        assert!(errs("//@ oven: time<=1\n")[0].contains("unknown budget"));
        assert!(errs("//@ oven: steps<=1 steps<=2\n")[0].contains("duplicate"));
    }

    #[test]
    fn errors_are_reported_with_lines() {
        let e = errs(concat!(
            "//@ expect: compile-maybe\n",
            "//@ frobnicate: 1\n",
            "//@ std: c++99\n",
            "//@ diag: 1:x error \"m\"\n",
            "//@ diag: 1:1 bogus \"m\"\n",
            "//@ diag: 1:1 error m\n",
            "//@ diag: 1:1 error \"m\" trailing\n",
            "//@ exit: 300\n",
            "//@ strict-diags: yes\n",
            "//@ skip:\n",
            "//@ skip(gcc): x\n",
            "//@ std(kiln): c++20\n",
            "//@ args: \"unterminated\n",
        ));
        assert_eq!(e.len(), 14, "{e:#?}");
        assert!(e[0].starts_with("line 1: unknown expectation"));
        assert!(e[1].starts_with("line 2: unknown directive `frobnicate`"));
        assert!(e[2].starts_with("line 3: unknown std"));
        assert!(e[3].contains("bad column"));
        assert!(e[4].contains("unknown severity"));
        assert!(e[5].contains("double-quoted"));
        assert!(e[6].contains("unexpected text after message"));
        assert!(e[7].contains("0..=255"));
        assert!(e[8].contains("takes no value"));
        assert!(e[9].contains("requires a reason"));
        assert!(e[10].contains("unknown compiler `gcc`"));
        assert!(e[11].contains("does not take a compiler qualifier"));
        assert!(e[12].contains("unterminated"));
        // exit is also run-only, reported at finish.
        assert!(e[13].contains("`exit` only applies to `expect: run`"));
    }

    #[test]
    fn misplaced_and_inapplicable_directives() {
        let e = errs("int x;\n//@ expect: run\n");
        assert_eq!(
            e,
            [
                "line 2: `//@` directive after the header (directives must precede the first line of code)"
            ]
        );
        let e = errs("//@ stdout: x\n");
        assert_eq!(e, ["line 1: `stdout` only applies to `expect: run`"]);
        let e = errs("//@ oracle: clang\n");
        assert_eq!(e, ["line 1: `oracle` only applies to `run` and `pp` tests"]);
        let e = errs("//@ diag: 1 note \"x\"\n");
        assert!(e[0].contains("must follow"));
        let e = errs("//@ expect: run\n//@ stdout: a\n//@ stdout-file: b\n");
        assert!(e[0].contains("mutually exclusive"));
        let e = errs("//@ expect: run\n//@ expect: pp\n//@ std: c++17\n//@ std: c++20\n");
        assert_eq!(e.len(), 2);
        let e = errs("//@ xfail: a\n//@ xfail: b\n");
        assert!(e[0].contains("duplicate `xfail`"));
        let e = errs("//@ expect\n");
        assert!(e[0].contains("requires a value"));
    }

    #[test]
    fn c_std_values() {
        assert_eq!(parse("//@ std: c11\n", Lang::C).unwrap().std, "c11");
        assert!(parse("//@ std: c++20\n", Lang::C).is_err());
        assert!(parse("//@ std: c11\n", Lang::Cxx).is_err());
    }

    #[test]
    fn arg_splitting() {
        assert_eq!(split_args("  a  b\tc ").unwrap(), ["a", "b", "c"]);
        assert_eq!(
            split_args(r#"-D'X=1 2' "q\"x" a\ b ''"#).unwrap(),
            ["-DX=1 2", "q\"x", "a b", ""]
        );
        assert!(split_args("'open").is_err());
        assert!(split_args("x\\").is_err());
        assert_eq!(split_args("").unwrap(), Vec::<String>::new());
    }
}
