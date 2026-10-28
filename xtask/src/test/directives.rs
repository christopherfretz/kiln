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
    /// `-fsyntax-only -fdump-ast`; compare the dump with `<test>.expected`
    /// (`spec/ast-dump.md` §7).
    Ast,
}

impl Expect {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "compile-ok" => Self::CompileOk,
            "compile-fail" => Self::CompileFail,
            "run" => Self::Run,
            "pp" => Self::Pp,
            "ast" => Self::Ast,
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
            Self::Ast => "ast",
        }
    }
}

/// How a `pp` test's output is compared (`pp-compare:`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PpCompare {
    /// Re-lex both outputs with `kiln_lex` and compare pp-token sequences.
    #[default]
    Tokens,
    /// Whitespace-normalized text, line by line.
    Text,
}

impl PpCompare {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "tokens" => Self::Tokens,
            "text" => Self::Text,
            _ => return None,
        })
    }
}

/// The `%{name}` placeholders `args:` may use, substituted by the runner
/// (`spec/harness.md` "Placeholders").
pub const PLACEHOLDERS: &[&str] = &["repo_root", "resource_dir", "test_dir"];

/// Checks that every `%{...}` in `arg` names a known placeholder.
fn check_placeholders(arg: &str) -> Result<(), String> {
    let mut rest = arg;
    while let Some(i) = rest.find("%{") {
        let after = &rest[i + 2..];
        let Some(end) = after.find('}') else {
            return Err(format!("unterminated placeholder in `{arg}`"));
        };
        let name = &after[..end];
        if !PLACEHOLDERS.contains(&name) {
            return Err(format!(
                "unknown placeholder `%{{{name}}}` (known: {})",
                PLACEHOLDERS.iter().map(|p| format!("%{{{p}}}")).collect::<Vec<_>>().join(", ")
            ));
        }
        rest = &after[end + 1..];
    }
    Ok(())
}

/// Replaces the placeholders in `arg`; `value` maps a known name to its
/// value. Unknown names were rejected by the parser and are left alone.
pub fn substitute(arg: &str, value: impl Fn(&str) -> Option<String>) -> String {
    let mut out = String::with_capacity(arg.len());
    let mut rest = arg;
    while let Some(i) = rest.find("%{") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        if let Some((end, v)) = after.find('}').and_then(|end| Some((end, value(&after[..end])?))) {
            out.push_str(&v);
            rest = &after[end + 1..];
        } else {
            out.push_str("%{");
            rest = after;
        }
    }
    out.push_str(rest);
    out
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

/// Standard input of a `run` test's binary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StdinInput {
    /// The `run-stdin:` lines, each `\n`-terminated.
    Inline(String),
    /// `run-stdin-file:`, relative to the directory containing the test.
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
    /// Arguments passed to the test binary (and the oracle's).
    pub run_args: Vec<String>,
    /// Standard input of the test binary (and the oracle's); `None` is empty.
    pub stdin: Option<StdinInput>,
    /// Extra environment variables of the test binary (and the oracle's), in
    /// directive order; names are unique.
    pub env: Vec<(String, String)>,
    /// Compare against the oracle compiler (default: on for `run`/`pp`).
    pub oracle: bool,
    /// `oven:` budgets: at most one unlocated, at most one per location.
    pub oven: Vec<OvenBudget>,
    pub skip: Vec<Marker>,
    pub xfail: Vec<Marker>,
    /// `pp-compare:`: how `pp` output is compared (default token-wise).
    pub pp_compare: PpCompare,
    /// `pp-linemarkers`: preprocess without `-P` and compare line markers.
    pub pp_linemarkers: bool,
    /// `ast-filter:`: dump only the declarations whose qualified name
    /// contains this substring (`-fdump-ast-filter=`).
    pub ast_filter: Option<String>,
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
        !self.oven.is_empty()
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
    run_args: Vec<String>,
    stdin_lines: Option<String>,
    stdin_file: Option<PathBuf>,
    env: Vec<(String, String)>,
    oracle: Option<bool>,
    oven: Vec<OvenBudget>,
    oven_lines: Vec<usize>,
    skip: Vec<Marker>,
    xfail: Vec<Marker>,
    pp_compare: Option<PpCompare>,
    pp_linemarkers: bool,
    ast_filter: Option<String>,
    /// Lines of directives whose validity depends on `expect`.
    run_only: Vec<(usize, &'static str)>,
    pp_only: Vec<(usize, &'static str)>,
    ast_only: Vec<(usize, &'static str)>,
    oracle_line: usize,
    stdout_file_line: usize,
    stdin_file_line: usize,
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
            run_args: Vec::new(),
            stdin_lines: None,
            stdin_file: None,
            env: Vec::new(),
            oracle: None,
            oven: Vec::new(),
            oven_lines: Vec::new(),
            skip: Vec::new(),
            xfail: Vec::new(),
            pp_compare: None,
            pp_linemarkers: false,
            ast_filter: None,
            run_only: Vec::new(),
            pp_only: Vec::new(),
            ast_only: Vec::new(),
            oracle_line: 0,
            stdout_file_line: 0,
            stdin_file_line: 0,
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
                    format!("unknown expectation `{value}` (compile-ok|compile-fail|run|pp|ast)"),
                ),
            },
            "std" => self.std_directive(line, value),
            "args" => match split_args(value) {
                Ok(mut a) => {
                    for e in a.iter().filter_map(|a| check_placeholders(a).err()) {
                        self.err(line, e);
                    }
                    self.args.append(&mut a);
                }
                Err(e) => self.err(line, e),
            },
            "pp-compare" => {
                self.pp_only.push((line, "pp-compare"));
                match PpCompare::parse(value) {
                    Some(_) if self.pp_compare.is_some() => {
                        self.err(line, "duplicate `pp-compare`");
                    }
                    Some(c) => self.pp_compare = Some(c),
                    None => self.err(line, format!("unknown pp-compare `{value}` (tokens|text)")),
                }
            }
            "ast-filter" => {
                self.ast_only.push((line, "ast-filter"));
                if self.ast_filter.is_some() {
                    self.err(line, "duplicate `ast-filter`");
                } else if value.is_empty() || value.contains(char::is_whitespace) {
                    self.err(line, "`ast-filter` requires a substring without whitespace");
                } else {
                    self.ast_filter = Some(value.to_owned());
                }
            }
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
            "run-args" => {
                self.run_only.push((line, "run-args"));
                match split_args(value) {
                    Ok(mut a) => self.run_args.append(&mut a),
                    Err(e) => self.err(line, e),
                }
            }
            "run-stdin" => {
                self.run_only.push((line, "run-stdin"));
                let s = self.stdin_lines.get_or_insert_with(String::new);
                s.push_str(value);
                s.push('\n');
            }
            "run-stdin-file" => self.stdin_file_directive(line, value),
            "env" => self.env_directive(line, value),
            "oracle" => self.oracle_directive(line, value),
            "oven" => match parse_oven(value) {
                Ok(b) if self.oven.iter().any(|o| o.at == b.at) => match b.at {
                    Some(at) => self.err(line, format!("duplicate `oven` for location {at}")),
                    None => self.err(line, "duplicate `oven` without a location"),
                },
                Ok(b) => {
                    self.oven.push(b);
                    self.oven_lines.push(line);
                }
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
        if name == "pp-linemarkers" {
            self.pp_only.push((line, "pp-linemarkers"));
            if value.is_some() {
                self.err(line, "`pp-linemarkers` takes no value");
            } else if self.pp_linemarkers {
                self.err(line, "duplicate `pp-linemarkers`");
            }
            self.pp_linemarkers = true;
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

    fn stdin_file_directive(&mut self, line: usize, value: &str) {
        self.run_only.push((line, "run-stdin-file"));
        if self.stdin_file.is_some() {
            self.err(line, "duplicate `run-stdin-file`");
        } else if value.is_empty() {
            self.err(line, "`run-stdin-file` requires a path");
        } else {
            self.stdin_file = Some(PathBuf::from(value));
            self.stdin_file_line = line;
        }
    }

    /// `env: NAME=value`; the value is the rest of the line, verbatim.
    fn env_directive(&mut self, line: usize, value: &str) {
        self.run_only.push((line, "env"));
        let Some((name, v)) = value.split_once('=') else {
            self.err(line, format!("expected `env: NAME=value`, got `{value}`"));
            return;
        };
        let valid = name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !valid {
            self.err(
                line,
                format!("bad environment variable name `{name}` ([A-Za-z_][A-Za-z0-9_]*)"),
            );
        } else if self.env.iter().any(|(n, _)| n == name) {
            self.err(line, format!("duplicate `env` for `{name}`"));
        } else {
            self.env.push((name.to_owned(), v.to_owned()));
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
        if expect != Expect::Pp {
            for (line, key) in std::mem::take(&mut self.pp_only) {
                self.err(line, format!("`{key}` only applies to `expect: pp`"));
            }
        }
        if expect != Expect::Ast {
            for (line, key) in std::mem::take(&mut self.ast_only) {
                self.err(line, format!("`{key}` only applies to `expect: ast`"));
            }
        }
        if self.oracle.is_some() && !matches!(expect, Expect::Run | Expect::Pp | Expect::Ast) {
            self.err(self.oracle_line, "`oracle` only applies to `run`, `pp` and `ast` tests");
        }
        if expect == Expect::Pp {
            for line in std::mem::take(&mut self.oven_lines) {
                self.err(line, "`oven` does not apply to `pp` tests (nothing is evaluated)");
            }
        }
        let stdin = match (self.stdin_lines.take(), self.stdin_file.take()) {
            (Some(_), Some(_)) => {
                let line = self.stdin_file_line;
                self.err(line, "`run-stdin` and `run-stdin-file` are mutually exclusive");
                None
            }
            (Some(s), None) => Some(StdinInput::Inline(s)),
            (None, Some(p)) => Some(StdinInput::File(p)),
            (None, None) => None,
        };
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
            run_args: self.run_args,
            stdin,
            env: self.env,
            oracle: self.oracle.unwrap_or(matches!(expect, Expect::Run | Expect::Pp)),
            oven: self.oven,
            skip: self.skip,
            xfail: self.xfail,
            pp_compare: self.pp_compare.unwrap_or_default(),
            pp_linemarkers: self.pp_linemarkers,
            ast_filter: self.ast_filter,
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
            | "run-args"
            | "run-stdin"
            | "run-stdin-file"
            | "env"
            | "oracle"
            | "oven"
            | "skip"
            | "xfail"
            | "pp-compare"
            | "ast-filter"
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
        use crate::util::header::{Counter, OvenLoc};
        let s = ok("//@ oven: bytes<=4096 steps<=100\n");
        assert_eq!(
            s.oven,
            [OvenBudget { at: None, limits: vec![(Counter::Steps, 100), (Counter::Bytes, 4096)] }]
        );
        assert!(s.is_kiln_specific());
        assert!(!ok("int x;\n").is_kiln_specific());
        let s = ok("//@ oven: evaluations<=3\n//@ oven: 12:1 steps<=5\n//@ oven: 13 calls<=1\n");
        assert_eq!(s.oven.len(), 3);
        assert_eq!(s.oven[1].at, Some(OvenLoc { line: 12, col: Some(1) }));
        assert!(errs("//@ oven: steps<100\n")[0].contains("bad budget"));
        assert!(errs("//@ oven: time<=1\n")[0].contains("unknown budget"));
        assert!(errs("//@ oven: steps<=1 steps<=2\n")[0].contains("duplicate"));
        assert!(errs("//@ oven: steps<=1\n//@ oven: bytes<=2\n")[0].contains("without a location"));
        assert!(
            errs("//@ oven: 4:2 steps<=1\n//@ oven: 4:2 bytes<=2\n")[0]
                .contains("duplicate `oven` for location 4:2")
        );
        // Different locations, including a line and a line:col on that line, are fine.
        assert_eq!(ok("//@ oven: 4 steps<=1\n//@ oven: 4:2 bytes<=2\n").oven.len(), 2);
        assert_eq!(
            errs("//@ expect: pp\n//@ oven: steps<=1\n"),
            ["line 2: `oven` does not apply to `pp` tests (nothing is evaluated)"]
        );
    }

    #[test]
    fn run_inputs() {
        let s = ok(concat!(
            "//@ expect: run\n",
            "//@ run-args: a 'b c'\n",
            "//@ run-args: -x\n",
            "//@ run-stdin: line one\n",
            "//@ run-stdin:\n",
            "//@ env: KILN_A=1\n",
            "//@ env: _B=x=y z\n",
            "//@ env: EMPTY=\n",
        ));
        assert_eq!(s.run_args, ["a", "b c", "-x"]);
        assert_eq!(s.stdin, Some(StdinInput::Inline("line one\n\n".into())));
        assert_eq!(
            s.env,
            [
                ("KILN_A".to_owned(), "1".to_owned()),
                ("_B".to_owned(), "x=y z".to_owned()),
                ("EMPTY".to_owned(), String::new())
            ]
        );
        let s = ok("//@ expect: run\n//@ run-stdin-file: Inputs/in.txt\n");
        assert_eq!(s.stdin, Some(StdinInput::File("Inputs/in.txt".into())));
        let s = ok("//@ expect: run\n");
        assert!(s.run_args.is_empty() && s.stdin.is_none() && s.env.is_empty());

        let e = errs(concat!(
            "//@ expect: run\n",
            "//@ run-stdin: a\n",
            "//@ run-stdin-file: f\n",
            "//@ run-stdin-file: g\n",
            "//@ env: 1X=a\n",
            "//@ env: NOEQ\n",
            "//@ env: A=1\n",
            "//@ env: A=2\n",
            "//@ run-args: 'open\n",
        ));
        assert_eq!(e.len(), 6, "{e:#?}");
        assert!(e[0].contains("duplicate `run-stdin-file`"));
        assert!(e[1].contains("bad environment variable name `1X`"));
        assert!(e[2].contains("expected `env: NAME=value`"));
        assert!(e[3].contains("duplicate `env` for `A`"));
        assert!(e[4].contains("unterminated"));
        assert!(e[5].starts_with("line 3:") && e[5].contains("mutually exclusive"));
        let e = errs("//@ run-args: x\n//@ run-stdin: y\n//@ env: A=b\n");
        assert_eq!(e.len(), 3, "{e:#?}");
        assert!(errs("//@ run-stdin-file: z\n")[0].contains("only applies to `expect: run`"));
        for (k, msg) in ["run-args", "run-stdin", "env"].iter().zip(&e) {
            assert!(msg.contains(&format!("`{k}` only applies to `expect: run`")), "{msg}");
        }
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
        assert_eq!(e, ["line 1: `oracle` only applies to `run`, `pp` and `ast` tests"]);
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
    fn pp_directives_and_placeholders() {
        let s = ok("//@ expect: pp\n");
        assert_eq!((s.pp_compare, s.pp_linemarkers), (PpCompare::Tokens, false));
        let s = ok("//@ expect: pp\n//@ pp-compare: text\n//@ pp-linemarkers\n");
        assert_eq!((s.pp_compare, s.pp_linemarkers), (PpCompare::Text, true));
        let e = errs("//@ pp-compare: text\n//@ pp-linemarkers\n");
        assert_eq!(
            e,
            [
                "line 1: `pp-compare` only applies to `expect: pp`",
                "line 2: `pp-linemarkers` only applies to `expect: pp`"
            ]
        );
        let e = errs("//@ expect: pp\n//@ pp-compare: bytes\n//@ pp-linemarkers: yes\n");
        assert!(e[0].contains("unknown pp-compare `bytes`"), "{e:?}");
        assert!(e[1].contains("takes no value"), "{e:?}");

        let s = ok("//@ args: -I%{test_dir}/Inputs -resource-dir %{resource_dir}\n");
        assert_eq!(s.args, ["-I%{test_dir}/Inputs", "-resource-dir", "%{resource_dir}"]);
        let e = errs("//@ args: -I%{tests_dir} %{repo_root\n");
        assert!(e[0].contains("unknown placeholder `%{tests_dir}`"), "{e:?}");
        assert!(e[1].contains("unterminated placeholder"), "{e:?}");
        let v = |n: &str| (n == "repo_root").then(|| "/r".to_owned());
        assert_eq!(substitute("-I%{repo_root}/x%{repo_root}", v), "-I/r/x/r");
        assert_eq!(substitute("%{other}%{repo_root}%{", v), "%{other}/r%{");
        assert_eq!(substitute("plain", v), "plain");
    }

    #[test]
    fn ast_directives() {
        let s = ok("//@ expect: ast\n");
        assert_eq!(s.expect, Expect::Ast);
        assert_eq!(s.ast_filter, None);
        assert!(!s.oracle, "oracle is off by default for ast (the golden is the contract)");
        let s = ok("//@ expect: ast\n//@ ast-filter: ns::f\n//@ oracle: clang\n");
        assert_eq!(s.ast_filter.as_deref(), Some("ns::f"));
        assert!(s.oracle);
        assert_eq!(
            errs("//@ ast-filter: f\n"),
            ["line 1: `ast-filter` only applies to `expect: ast`"]
        );
        let e = errs("//@ expect: ast\n//@ ast-filter: a\n//@ ast-filter: b\n");
        assert_eq!(e, ["line 3: duplicate `ast-filter`"]);
        let e = errs("//@ expect: ast\n//@ ast-filter: c d\n");
        assert!(e[0].contains("without whitespace"), "{e:?}");
        assert!(errs("//@ ast-filter\n")[0].contains("requires a value"));
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
