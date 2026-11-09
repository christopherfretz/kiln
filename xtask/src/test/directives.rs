//! The `//@` directive header of a conformance test (`spec/harness.md`,
//! `docs/VALIDATION.md` "Conformance test format").
//!
//! The header is the leading run of lines that are blank or `//` comments.
//! Every line in it whose trimmed text starts with `//@` is a directive of the
//! form `key: value` or (for flags) a bare `key`. A `//@` line after the header
//! is an error, so a misplaced directive can never be silently ignored.

use std::fmt;
use std::path::{Path, PathBuf};

pub use crate::util::header::{
    self, C_STDS, CXX_STDS, DirectiveError, OvenBudget, parse_oven, split_args,
};
pub use crate::util::{ClangVersion, CompilerKind, parse_version};

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
    /// `-fsyntax-only -fdump-record-layouts`; compare the dump with the
    /// oracle's `-fdump-record-layouts-complete`, block by block.
    Layout,
    /// `-fsyntax-only -fdump-vtable-layouts`; compare the dump with the
    /// oracle's (emitted at code generation) as a set of blocks.
    Vtables,
    /// `-S -emit-llvm -O0`; compare the module with the oracle's, exactly
    /// after dropping comments and the producer lines (WP-386).
    Ir,
    /// `-fsyntax-only` ends normally (exit 0 or 1): errors are fine, a
    /// crash, a hang or an internal-error exit code is not.
    NoCrash,
    /// `-fsyntax-only -fkiln-dump-cfg`; compare the dump with
    /// `<test>.expected`, and its shape with the oracle's `debug.DumpCFG`
    /// (spec/harness.md "cfg tests").
    Cfg,
    /// Compile and link at `-O0 -g`, then drive the binary with `gdb`
    /// and/or `lldb` scripts and check their output (spec/harness.md
    /// "Debugger tests"; WP-814).
    Debugger,
    /// kiln only: the compile crashes on purpose (`-fkiln-crash-for-testing=`)
    /// and its internal-compiler-error banner and crash reproducer are
    /// checked (spec/harness.md "ice tests"; WP-911).
    Ice,
}

impl Expect {
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "compile-ok" => Self::CompileOk,
            "compile-fail" => Self::CompileFail,
            "run" => Self::Run,
            "pp" => Self::Pp,
            "ast" => Self::Ast,
            "layout" => Self::Layout,
            "vtables" => Self::Vtables,
            "ir" => Self::Ir,
            "no-crash" => Self::NoCrash,
            "cfg" => Self::Cfg,
            "debugger" => Self::Debugger,
            "ice" => Self::Ice,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::CompileOk => "compile-ok",
            Self::CompileFail => "compile-fail",
            Self::Run => "run",
            Self::Pp => "pp",
            Self::Ast => "ast",
            Self::Layout => "layout",
            Self::Vtables => "vtables",
            Self::Ir => "ir",
            Self::NoCrash => "no-crash",
            Self::Cfg => "cfg",
            Self::Debugger => "debugger",
            Self::Ice => "ice",
        }
    }
}

/// A `debugger` test's script for one debugger: the `gdb:` / `lldb:`
/// commands in order, and the `gdb-check:` / `lldb-check:` substrings its
/// output must contain on successive lines, in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DebuggerScript {
    pub commands: Vec<String>,
    pub checks: Vec<String>,
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
pub const PLACEHOLDERS: &[&str] = &["gcc_toolchain", "repo_root", "resource_dir", "test_dir"];

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

/// `diag: <line>[:<col>] <severity> "<substring>" [dup(clang)]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagDirective {
    pub line: u32,
    /// `None` when the directive omits the column (any column matches).
    pub col: Option<u32>,
    pub severity: Severity,
    pub message: String,
    /// `dup(clang)`: clang prints this group twice, kiln once (its engine
    /// deduplicates, spec/diag.md §7.4). Only on a primary directive.
    pub clang_dup: bool,
    /// 1-based line of the directive itself, for error messages.
    pub directive_line: usize,
}

impl fmt::Display for DiagDirective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.col {
            Some(c) => write!(f, "{}:{}", self.line, c)?,
            None => write!(f, "{}", self.line)?,
        }
        write!(f, " {} {:?}", self.severity.as_str(), self.message)?;
        if self.clang_dup {
            write!(f, " dup(clang)")?;
        }
        Ok(())
    }
}

/// A `skip`/`xfail` marker: `skip: reason` (all compilers) or
/// `skip(kiln): reason` / `skip(clang): reason` / `skip(gcc): reason`.
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
#[allow(clippy::struct_excessive_bools, reason = "independent directives")]
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
    /// `oracle: gcc`: compare kiln's verdict (and a `run` test's stdout and
    /// exit status) with the GCC oracle instead of clang
    /// (`spec/harness.md` "GCC oracle").
    pub gcc: bool,
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
    /// `cfg-diverges: <function> <reason>`: the functions of a `cfg` test
    /// whose graph differs from clang's by construction (the golden holds
    /// kiln's); their shapes are not compared with the oracle's.
    pub cfg_diverges: Vec<(String, String)>,
    /// `inst-report:`: the golden NDJSON template instantiation report
    /// (`-fkiln-instantiation-report=json`), relative to the test's
    /// directory; compared with the `*_us` timings masked.
    pub inst_report: Option<PathBuf>,
    /// `ast-implicit`: include implicit declarations (`-fdump-ast=implicit`,
    /// `xtask clang-ast --implicit`).
    pub ast_implicit: bool,
    /// `mem<=N`: the address-space cap, in MiB, of every process the test
    /// runs (lowers `--mem-limit`; `spec/harness.md` "Resource limits").
    pub mem_mib: Option<u64>,
    /// `requires: target-<arch>`: the architecture the test's effective
    /// target (its `--target`, else the host) must have; otherwise the test
    /// is a SKIP.
    pub requires_target: Option<String>,
    /// `requires: host-<feature>` (WP-810): a CPU feature of
    /// [`HOST_FEATURES`] the host the harness runs on must have, the test
    /// targeting the host's architecture; otherwise the test is a SKIP.
    pub requires_host: Option<String>,
    /// `requires: sanitizer-runtime` (WP-1032): every clang that links the
    /// test's program must find the undefined-behaviour and address sanitizer runtimes (compiler-rt);
    /// otherwise the test is a SKIP. The runner probes for them.
    pub requires_sanitizer_runtime: bool,
    /// `verify`: the outcome is clang's `-verify` check of the test's
    /// `expected-*` comments, under the compiler and the oracle.
    pub verify: bool,
    /// `requires: clang>=X[.Y[.Z]]`: the least version of every clang the
    /// test runs (the compiler under test, the oracle); otherwise a SKIP.
    pub requires_clang: Option<ClangVersion>,
    /// `requires: clang<X[.Y[.Z]]`: every clang the test runs must be older
    /// (an expectation a later release changed, kept until kiln follows it;
    /// docs/status/wp-184-oracle-switch.md "Residual"); otherwise a SKIP.
    pub requires_clang_below: Option<ClangVersion>,
    /// `module: <name>=<path>`: the module interface units the test
    /// imports, in dependency order, paths relative to the test's directory
    /// (`spec/harness.md` "Modules"; the same lines `ir_vs_clang` reads).
    pub modules: Vec<(String, PathBuf)>,
    /// `unit: <path>`: further translation units (an implementation unit,
    /// any other TU) a `run` test compiles separately, against the module
    /// files, and links (`spec/harness.md` "Modules").
    pub units: Vec<PathBuf>,
    /// `two-step`: kiln, too, compiles the `module:` units to module files
    /// (the test then imports those) instead of reading their sources.
    /// Implied for `run` tests.
    pub two_step: bool,
    /// `min-oracle: N`: every clang the test runs must be release N or
    /// later; otherwise a SKIP (`ir_vs_clang` compares the case only under
    /// such an oracle).
    pub min_oracle: Option<u32>,
    /// `requires: llvm>=N`: the least LLVM major of every backend that
    /// consumes the test's IR (kiln's `KILN_LLVM_DIR`, each clang run);
    /// otherwise a SKIP. For IR goldens whose syntax an older LLVM rejects.
    pub requires_llvm: Option<u32>,
    /// `requires: llvm<N`: every such backend must be older; otherwise a
    /// SKIP (a workaround only an older backend needs).
    pub requires_llvm_below: Option<u32>,
    /// `meta-header: skip <reason>`: left out of the header-differential
    /// mode (`xtask test --meta-header`; `spec/harness.md` "Header-
    /// differential mode"), with the reason it cannot run there.
    pub meta_header_skip: Option<String>,
    /// `gdb:` / `gdb-check:` (`expect: debugger`).
    pub gdb: DebuggerScript,
    /// `lldb:` / `lldb-check:` (`expect: debugger`).
    pub lldb: DebuggerScript,
    /// `ice:` (`expect: ice`): substrings the crash report's stderr must
    /// contain in order, each after the end of the previous one.
    pub ice: Vec<String>,
}

/// A clang that a test runs, for `requires: clang>=`: what it is to the
/// test (`the compiler`, `the oracle`) and the version its `--version` line
/// names (`None`: none could be parsed, which never meets a requirement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClangRun<'a> {
    pub role: &'a str,
    pub version: Option<ClangVersion>,
}

/// A backend that consumes a test's IR, for `requires: llvm>=`: what it is
/// to the test (`kiln's backend`, `the compiler`, `the oracle`) and its LLVM
/// major (`None`: unknown, which never meets a requirement).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LlvmRun<'a> {
    pub role: &'a str,
    pub major: Option<u32>,
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

    /// The `--target` triple the test's `args:` pin, if any: the last of
    /// `--target=<t>`, `--target <t>` and `-target <t>`, as for clang.
    pub fn target_triple(&self) -> Option<&str> {
        let mut found = None;
        let mut args = self.args.iter();
        while let Some(a) = args.next() {
            if let Some(t) = a.strip_prefix("--target=") {
                found = Some(t);
            } else if a == "--target" || a == "-target" {
                found = args.next().map(String::as_str).or(found);
            }
        }
        found
    }

    /// The architecture the test compiles for (`std::env::consts::ARCH`
    /// spelling): that of its `--target` triple, else `host`, the default
    /// target of kiln and clang.
    pub fn target_arch<'a>(&'a self, host: &'a str) -> &'a str {
        self.target_triple().map_or(host, triple_arch)
    }

    /// Why the test cannot run, if it cannot: `host` is the harness's
    /// architecture (`std::env::consts::ARCH`), `clangs` the clangs the test
    /// would run, `llvms` the LLVM backends that would consume its IR
    /// (`spec/harness.md`, `requires:`).
    pub fn unmet_requirement(
        &self,
        host: &str,
        clangs: &[ClangRun<'_>],
        llvms: &[LlvmRun<'_>],
    ) -> Option<String> {
        if let Some(want) = self.requires_host.as_deref() {
            let arch = self.target_arch(host);
            let feats = host_features();
            if arch != host {
                return Some(format!(
                    "requires host-{want}; the test targets {arch}, not the host"
                ));
            }
            if !feats.iter().any(|f| f == want) {
                return Some(format!("requires host-{want}; the host CPU lacks {want}"));
            }
        }
        if let Some(want) = self.requires_target.as_deref() {
            let arch = self.target_arch(host);
            if want != arch {
                return Some(match self.target_triple() {
                    Some(t) => format!("requires target-{want}; the test targets {arch} (`{t}`)"),
                    None => format!("requires target-{want}; the default target is {arch}"),
                });
            }
        }
        let at_least = self.requires_clang.and_then(|want| {
            let need = format!("requires clang>={}", version_str(want));
            clangs.iter().find_map(|c| match c.version {
                Some(v) if v >= want => None,
                Some(v) => Some(format!("{need}; {} is clang {}", c.role, version_str(v))),
                None => Some(format!("{need}; {} names no clang version", c.role)),
            })
        });
        let at_least = at_least.or_else(|| {
            let n = self.min_oracle?;
            clangs.iter().find_map(|c| match c.version {
                Some(v) if v[0] >= n => None,
                Some(v) => Some(format!("min-oracle {n}; {} is clang {}", c.role, version_str(v))),
                None => Some(format!("min-oracle {n}; {} names no clang version", c.role)),
            })
        });
        let below = || {
            let below = self.requires_clang_below?;
            let need = format!("requires clang<{}", version_str(below));
            clangs.iter().find_map(|c| match c.version {
                Some(v) if v < below => None,
                Some(v) => Some(format!("{need}; {} is clang {}", c.role, version_str(v))),
                None => Some(format!("{need}; {} names no clang version", c.role)),
            })
        };
        let llvm = |bound: Option<u32>, op: &str, ok: fn(u32, u32) -> bool| {
            let want = bound?;
            llvms.iter().find_map(|l| match l.major {
                Some(m) if ok(m, want) => None,
                Some(m) => Some(format!("requires llvm{op}{want}; {} is LLVM {m}", l.role)),
                None => Some(format!("requires llvm{op}{want}; {} has no known LLVM", l.role)),
            })
        };
        at_least
            .or_else(below)
            .or_else(|| llvm(self.requires_llvm, ">=", |m, w| m >= w))
            .or_else(|| llvm(self.requires_llvm_below, "<", |m, w| m < w))
    }

    /// Does this test use directives only kiln can check?
    pub fn is_kiln_specific(&self) -> bool {
        !self.oven.is_empty() || self.inst_report.is_some()
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

#[allow(clippy::struct_excessive_bools)]
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
    cfg_diverges: Vec<(String, String)>,
    cfg_lines: Vec<usize>,
    inst_report: Option<PathBuf>,
    inst_report_line: usize,
    ast_implicit: bool,
    mem_mib: Option<u64>,
    requires_target: Option<String>,
    requires_host: Option<String>,
    requires_sanitizer_runtime: bool,
    /// Line of `verify` (0: absent).
    verify_line: usize,
    /// Line of the first `diag:` / `strict-diags` (0: none).
    diag_line: usize,
    requires_clang: Option<ClangVersion>,
    requires_clang_below: Option<ClangVersion>,
    modules: Vec<(String, PathBuf)>,
    /// Line of the first `module:` (0: none).
    module_line: usize,
    units: Vec<PathBuf>,
    /// Line of the first `unit:` (0: none).
    unit_line: usize,
    /// Line of `two-step` (0: absent).
    two_step_line: usize,
    min_oracle: Option<u32>,
    requires_llvm: Option<u32>,
    requires_llvm_below: Option<u32>,
    meta_header_skip: Option<String>,
    gdb: DebuggerScript,
    lldb: DebuggerScript,
    ice: Vec<String>,
    /// Lines of the `ice:` directives.
    ice_lines: Vec<usize>,
    /// Lines of directives whose validity depends on `expect`.
    run_only: Vec<(usize, &'static str)>,
    debugger_only: Vec<(usize, &'static str)>,
    pp_only: Vec<(usize, &'static str)>,
    ast_only: Vec<(usize, &'static str)>,
    oracle_line: usize,
    /// The line of `oracle: gcc` (0: none).
    gcc_line: usize,
    skip_gcc_line: usize,
    /// Line of the `skip(clang)` marker, 0 when there is none.
    skip_clang_line: usize,
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
            cfg_diverges: Vec::new(),
            cfg_lines: Vec::new(),
            meta_header_skip: None,
            inst_report: None,
            inst_report_line: 0,
            ast_implicit: false,
            mem_mib: None,
            requires_target: None,
            requires_host: None,
            requires_sanitizer_runtime: false,
            verify_line: 0,
            diag_line: 0,
            requires_clang: None,
            requires_clang_below: None,
            modules: Vec::new(),
            module_line: 0,
            units: Vec::new(),
            unit_line: 0,
            two_step_line: 0,
            min_oracle: None,
            requires_llvm: None,
            requires_llvm_below: None,
            run_only: Vec::new(),
            gdb: DebuggerScript::default(),
            lldb: DebuggerScript::default(),
            ice: Vec::new(),
            ice_lines: Vec::new(),
            debugger_only: Vec::new(),
            pp_only: Vec::new(),
            ast_only: Vec::new(),
            oracle_line: 0,
            gcc_line: 0,
            skip_gcc_line: 0,
            skip_clang_line: 0,
            stdout_file_line: 0,
            stdin_file_line: 0,
        }
    }

    fn err(&mut self, line: usize, message: impl Into<String>) {
        self.errors.push(DirectiveError { line, message: message.into() });
    }

    #[allow(clippy::too_many_lines, reason = "one flat match over the directive keys")]
    fn directive(&mut self, line: usize, key: &str, value: Option<&str>) {
        if key == "mem" || key.starts_with("mem<") || key.starts_with("mem=") {
            self.mem_directive(line, key, value);
            return;
        }
        let Some((name, qualifier, value)) = self.split_key(line, key, value) else {
            return;
        };
        match name {
            "expect" => match Expect::parse(value) {
                Some(e) if self.expect.is_none() => self.expect = Some(e),
                Some(_) => self.err(line, "duplicate `expect`"),
                None => self.err(
                    line,
                    format!("unknown expectation `{value}` (compile-ok|compile-fail|run|pp|ast|layout|vtables|ir|no-crash|cfg|debugger)"),
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
            "cfg-diverges" => {
                self.cfg_lines.push(line);
                match value.split_once(char::is_whitespace) {
                    Some((f, why)) if !why.trim().is_empty() => {
                        self.cfg_diverges.push((f.to_owned(), why.trim().to_owned()));
                    }
                    _ => self.err(line, "`cfg-diverges` requires a function name and a reason"),
                }
            }
            "meta-header" => match value.strip_prefix("skip") {
                _ if self.meta_header_skip.is_some() => self.err(line, "duplicate `meta-header`"),
                Some(why) if why.starts_with(char::is_whitespace) && !why.trim().is_empty() => {
                    self.meta_header_skip = Some(why.trim().to_owned());
                }
                _ => self.err(line, "expected `meta-header: skip <reason>`"),
            },
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
                    if self.diag_line == 0 {
                        self.diag_line = line;
                    }
                    if d.severity == Severity::Note && self.diags.is_empty() {
                        self.err(
                            line,
                            "a `note` diag must follow the error/warning/remark it belongs to",
                        );
                    }
                    if d.severity == Severity::Note && d.clang_dup {
                        self.err(line, "`dup(clang)` marks a whole group: put it on the primary");
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
            "inst-report" => {
                if self.inst_report.is_some() {
                    self.err(line, "duplicate `inst-report`");
                } else if value.is_empty() {
                    self.err(line, "`inst-report` requires a path");
                } else {
                    self.inst_report_line = line;
                    self.inst_report = Some(PathBuf::from(value));
                }
            }
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
            "gdb" | "gdb-check" | "lldb" | "lldb-check" => {
                let key = match name {
                    "gdb" => "gdb",
                    "gdb-check" => "gdb-check",
                    "lldb" => "lldb",
                    _ => "lldb-check",
                };
                self.debugger_only.push((line, key));
                if value.is_empty() {
                    self.err(line, format!("`{key}` requires a value"));
                }
                let script = if name.starts_with("gdb") { &mut self.gdb } else { &mut self.lldb };
                let list =
                    if name.ends_with("-check") { &mut script.checks } else { &mut script.commands };
                list.push(value.to_owned());
            }
            "ice" => {
                if value.is_empty() {
                    self.err(line, "`ice` requires a value");
                }
                self.ice.push(value.to_owned());
                self.ice_lines.push(line);
            }
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
            "requires" => self.requires_directive(line, value),
            "module" => self.module_directive(line, value),
            "unit" => {
                if value.is_empty() || value.contains(char::is_whitespace) {
                    self.err(line, "expected `unit: <path>`");
                } else if Path::new(value).is_absolute() {
                    self.err(line, "a `unit` path is relative to the test's directory");
                } else {
                    if self.unit_line == 0 {
                        self.unit_line = line;
                    }
                    self.units.push(PathBuf::from(value));
                }
            }
            "min-oracle" => match value.parse::<u32>() {
                Ok(_) if self.min_oracle.is_some() => self.err(line, "duplicate `min-oracle`"),
                Ok(n) if n > 0 => self.min_oracle = Some(n),
                _ => self.err(line, format!("`min-oracle` must be a clang major release, got `{value}`")),
            },
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
        if name == "verify" {
            if value.is_some() {
                self.err(line, "`verify` takes no value");
            } else if self.verify_line != 0 {
                self.err(line, "duplicate `verify`");
            } else {
                self.verify_line = line;
            }
            return None;
        }
        if name == "two-step" {
            if value.is_some() {
                self.err(line, "`two-step` takes no value");
            } else if self.two_step_line != 0 {
                self.err(line, "duplicate `two-step`");
            } else {
                self.two_step_line = line;
            }
            return None;
        }
        if name == "strict-diags" {
            if self.diag_line == 0 {
                self.diag_line = line;
            }
            if value.is_some() {
                self.err(line, "`strict-diags` takes no value");
            } else if self.strict_diags {
                self.err(line, "duplicate `strict-diags`");
            }
            self.strict_diags = true;
            return None;
        }
        if name == "ast-implicit" {
            self.ast_only.push((line, "ast-implicit"));
            if value.is_some() {
                self.err(line, "`ast-implicit` takes no value");
            } else if self.ast_implicit {
                self.err(line, "duplicate `ast-implicit`");
            }
            self.ast_implicit = true;
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

    /// `mem<=N`: a memory budget of N MiB (a positive integer), written as
    /// a bare key like the `oven:` budgets' items.
    fn mem_directive(&mut self, line: usize, key: &str, value: Option<&str>) {
        let n = key.strip_prefix("mem<=").map(str::trim);
        let parsed = match (n, value) {
            (Some(n), None) => n.parse::<u64>().ok().filter(|&n| n > 0),
            _ => None,
        };
        match parsed {
            Some(_) if self.mem_mib.is_some() => self.err(line, "duplicate `mem<=`"),
            Some(n) => self.mem_mib = Some(n),
            None => self.err(
                line,
                "a memory budget is written `mem<=N`, N a positive number of MiB (e.g. `//@ mem<=512`)",
            ),
        }
    }

    /// `module: <name>=<path>`: a module interface unit the test imports;
    /// each name at most once.
    fn module_directive(&mut self, line: usize, value: &str) {
        let Some((name, path)) = value.split_once('=').map(|(n, p)| (n.trim(), p.trim())) else {
            self.err(line, "expected `module: <name>=<path>`");
            return;
        };
        if name.is_empty() || path.is_empty() || name.contains(char::is_whitespace) {
            self.err(line, "expected `module: <name>=<path>`");
        } else if Path::new(path).is_absolute() {
            self.err(line, "a `module` path is relative to the test's directory");
        } else if self.modules.iter().any(|(n, _)| n == name) {
            self.err(line, format!("duplicate `module` `{name}`"));
        } else {
            if self.module_line == 0 {
                self.module_line = line;
            }
            self.modules.push((name.to_owned(), PathBuf::from(path)));
        }
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
            "none" | "gcc" => Some(false),
            _ => None,
        };
        match v {
            Some(_) if self.oracle.is_some() => self.err(line, "duplicate `oracle`"),
            Some(b) => {
                self.oracle = Some(b);
                self.oracle_line = line;
                if value == "gcc" {
                    self.gcc_line = line;
                }
            }
            None => self.err(line, format!("unknown oracle `{value}` (clang|gcc|none)")),
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

    /// `requires: target-<arch>` (`<arch>` one of [`TARGET_ARCHES`]),
    /// `requires: host-<feature>` (one of [`HOST_FEATURES`]),
    /// `requires: clang>=X[.Y[.Z]]`, `requires: clang<X[.Y[.Z]]`,
    /// `requires: llvm>=N`, `requires: llvm<N` or
    /// `requires: sanitizer-runtime`; each kind at most once.
    fn requires_directive(&mut self, line: usize, value: &str) {
        let usage = format!(
            "expected `requires: target-<arch>` (<arch> one of {}), `requires: clang>=X[.Y[.Z]]`, \
             `requires: clang<X[.Y[.Z]]`, `requires: llvm>=N`, `requires: llvm<N` or \
             `requires: sanitizer-runtime`",
            TARGET_ARCHES.join(" ")
        );
        if value == "sanitizer-runtime" {
            if self.requires_sanitizer_runtime {
                self.err(line, "duplicate `requires: sanitizer-runtime`");
            }
            self.requires_sanitizer_runtime = true;
        } else if let Some(f) = value.strip_prefix("host-") {
            if !HOST_FEATURES.contains(&f) {
                self.err(
                    line,
                    format!("unknown host feature `{f}` (one of {})", HOST_FEATURES.join(" ")),
                );
            } else if self.requires_host.is_some() {
                self.err(line, "duplicate `requires: host-`");
            } else {
                self.requires_host = Some(f.to_owned());
            }
        } else if let Some(a) = value.strip_prefix("target-") {
            if !TARGET_ARCHES.contains(&a) {
                self.err(line, usage);
            } else if self.requires_target.is_some() {
                self.err(line, "duplicate `requires: target-`");
            } else {
                self.requires_target = Some(a.to_owned());
            }
        } else if let Some(v) = value.strip_prefix("clang>=") {
            match parse_version(v.trim()) {
                None => self.err(line, usage),
                Some(_) if self.requires_clang.is_some() => {
                    self.err(line, "duplicate `requires: clang>=`");
                }
                Some(v) => self.requires_clang = Some(v),
            }
        } else if let Some(v) = value.strip_prefix("clang<") {
            match parse_version(v.trim()) {
                None => self.err(line, usage),
                Some(_) if self.requires_clang_below.is_some() => {
                    self.err(line, "duplicate `requires: clang<`");
                }
                Some(v) => self.requires_clang_below = Some(v),
            }
        } else if let Some((v, below)) = value
            .strip_prefix("llvm>=")
            .map(|v| (v, false))
            .or_else(|| value.strip_prefix("llvm<").map(|v| (v, true)))
        {
            let v = v.trim();
            let major = v.parse::<u32>().ok().filter(|_| v.bytes().all(|b| b.is_ascii_digit()));
            let slot = if below { &mut self.requires_llvm_below } else { &mut self.requires_llvm };
            match major {
                None => self.err(line, usage),
                Some(_) if slot.is_some() => {
                    let op = if below { "<" } else { ">=" };
                    self.err(line, format!("duplicate `requires: llvm{op}`"));
                }
                Some(m) => *slot = Some(m),
            }
        } else {
            self.err(line, usage);
        }
    }

    fn marker(&mut self, line: usize, name: &str, qualifier: Option<&str>, reason: &str) {
        let compiler = match qualifier {
            None => None,
            Some("kiln") => Some(CompilerKind::Kiln),
            Some("clang") => Some(CompilerKind::Clang),
            Some("gcc") => Some(CompilerKind::Gcc),
            Some(q) => {
                self.err(line, format!("unknown compiler `{q}` (kiln|clang|gcc)"));
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
        if name == "skip" && compiler == Some(CompilerKind::Clang) {
            self.skip_clang_line = line;
        }
        if name == "skip" && compiler == Some(CompilerKind::Gcc) {
            self.skip_gcc_line = line;
        }
        list.push(Marker { compiler, reason: reason.to_owned() });
    }

    /// Validates `verify` against `expect` and `diag:`; returns whether
    /// the test is a `verify` test.
    fn check_verify(&mut self, expect: Expect) -> bool {
        if self.verify_line == 0 {
            return false;
        }
        if expect != Expect::CompileOk {
            let line = self.verify_line;
            self.err(
                line,
                "`verify` only applies to `compile-ok` tests (the outcome is the -verify check)",
            );
        }
        if self.diag_line != 0 {
            let line = self.diag_line;
            self.err(line, "`diag:` and `strict-diags` do not apply to `verify` tests (use expected-* comments)");
        }
        true
    }

    /// The `run-stdin`/`run-stdin-file` and `stdout`/`stdout-file`
    /// pairs, each of which may name only one side.
    fn take_io(&mut self) -> (Option<StdinInput>, Option<StdoutExpect>) {
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
        (stdin, stdout)
    }

    /// An `expect: debugger` test drives at least one debugger, and each
    /// debugger it drives has both commands and checks.
    fn check_debugger(&mut self) {
        let first = self.debugger_only.first().map_or(1, |&(l, _)| l);
        if self.gdb.commands.is_empty() && self.lldb.commands.is_empty() {
            self.err(first, "`expect: debugger` requires `gdb:` or `lldb:` commands");
        }
        for (name, sc) in [("gdb", self.gdb.clone()), ("lldb", self.lldb.clone())] {
            if sc.commands.is_empty() != sc.checks.is_empty() {
                let what = if sc.commands.is_empty() {
                    format!("`{name}-check` without `{name}:` commands")
                } else {
                    format!("`{name}:` commands without a `{name}-check`")
                };
                self.err(first, what);
            }
        }
    }

    /// Validates `oracle: gcc` against `expect`, `verify` and `skip(gcc)`;
    /// returns whether the test has it.
    fn check_gcc(&mut self, expect: Expect, verify: bool) -> bool {
        if self.gcc_line == 0 {
            return false;
        }
        let line = self.gcc_line;
        if verify || !matches!(expect, Expect::Run | Expect::CompileOk | Expect::CompileFail) {
            self.err(
                line,
                "`oracle: gcc` only applies to `run`, `compile-ok` and `compile-fail` tests \
                 (not `verify`)",
            );
        }
        if self.skip_gcc_line != 0 {
            self.err(line, "`oracle: gcc` contradicts `skip(gcc)`");
        }
        true
    }

    /// Validates `skip(clang)` against `oracle` and `expect`; returns
    /// whether the test has it.
    fn check_skip_clang(&mut self, expect: Expect) -> bool {
        // The oracle is a clang: a test clang cannot compile is not
        // compared with it (`skip(clang)` implies `oracle: none`).
        if self.skip_clang_line == 0 {
            return false;
        }
        if self.oracle == Some(true) {
            self.err(self.oracle_line, "`oracle: clang` contradicts `skip(clang)`");
        }
        // `layout`, `vtables` and `ir` have no golden: without the oracle
        // there is nothing to compare kiln's output with.
        if matches!(expect, Expect::Layout | Expect::Vtables | Expect::Ir) {
            let line = self.skip_clang_line;
            self.err(
                line,
                "`skip(clang)` does not apply to `layout`, `vtables` and `ir` tests \
                 (the oracle is their only reference); use `skip:`",
            );
        }
        true
    }

    #[allow(clippy::too_many_lines, reason = "one flat pass of cross-directive checks")]
    fn finish(mut self) -> Result<TestSpec, Vec<DirectiveError>> {
        let expect = self.expect.unwrap_or(Expect::CompileOk);
        if expect != Expect::Run {
            for (line, key) in std::mem::take(&mut self.run_only) {
                self.err(line, format!("`{key}` only applies to `expect: run`"));
            }
        }
        if expect == Expect::Debugger {
            self.check_debugger();
        } else {
            for (line, key) in std::mem::take(&mut self.debugger_only) {
                self.err(line, format!("`{key}` only applies to `expect: debugger`"));
            }
        }
        if expect == Expect::Ice {
            if self.diag_line != 0 {
                let line = self.diag_line;
                self.err(line, "`diag` does not apply to `ice` tests (use `ice:`)");
            }
        } else {
            for line in std::mem::take(&mut self.ice_lines) {
                self.err(line, "`ice` only applies to `expect: ice`");
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
        if expect != Expect::Cfg {
            for line in std::mem::take(&mut self.cfg_lines) {
                self.err(line, "`cfg-diverges` only applies to `expect: cfg`");
            }
        }
        let verify = self.check_verify(expect);
        let gcc = self.check_gcc(expect, verify);
        if self.oracle.is_some()
            && !gcc
            && !verify
            && !matches!(expect, Expect::Run | Expect::Pp | Expect::Ast)
        {
            self.err(
                self.oracle_line,
                "`oracle` only applies to `run`, `pp`, `ast` and `verify` tests",
            );
        }
        let skip_clang = self.check_skip_clang(expect);
        if self.inst_report.is_some() && !matches!(expect, Expect::CompileOk | Expect::CompileFail)
        {
            let line = self.inst_report_line;
            self.err(line, "`inst-report` only applies to `compile-ok` and `compile-fail` tests");
        }
        if self.module_line != 0
            && (verify
                || !matches!(
                    expect,
                    Expect::CompileOk | Expect::CompileFail | Expect::NoCrash | Expect::Run
                ))
        {
            let line = self.module_line;
            self.err(
                line,
                "`module` only applies to `compile-ok`, `compile-fail`, `no-crash` and `run` tests",
            );
        }
        if self.unit_line != 0 && expect != Expect::Run {
            let line = self.unit_line;
            self.err(line, "`unit` only applies to `run` tests");
        }
        if self.two_step_line != 0 && (self.module_line == 0 || expect == Expect::Run) {
            let line = self.two_step_line;
            self.err(line, "`two-step` needs `module:` lines and is implied for `run` tests");
        }
        if expect == Expect::Pp {
            for line in std::mem::take(&mut self.oven_lines) {
                self.err(line, "`oven` does not apply to `pp` tests (nothing is evaluated)");
            }
        }
        let (stdin, stdout) = self.take_io();
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
            // `layout`, `vtables` and `ir` have no golden: the oracle is
            // their only reference, and `oracle:` does not apply to them.
            oracle: self.oracle.unwrap_or(
                if verify || matches!(expect, Expect::Run | Expect::Pp) {
                    !skip_clang
                } else {
                    matches!(expect, Expect::Layout | Expect::Vtables | Expect::Ir)
                },
            ),
            gcc,
            oven: self.oven,
            skip: self.skip,
            xfail: self.xfail,
            pp_compare: self.pp_compare.unwrap_or_default(),
            pp_linemarkers: self.pp_linemarkers,
            ast_filter: self.ast_filter,
            cfg_diverges: self.cfg_diverges,
            inst_report: self.inst_report,
            ast_implicit: self.ast_implicit,
            mem_mib: self.mem_mib,
            requires_target: self.requires_target,
            requires_host: self.requires_host,
            requires_sanitizer_runtime: self.requires_sanitizer_runtime,
            verify,
            requires_clang: self.requires_clang,
            requires_clang_below: self.requires_clang_below,
            modules: self.modules,
            units: self.units,
            two_step: self.two_step_line != 0,
            min_oracle: self.min_oracle,
            requires_llvm: self.requires_llvm,
            requires_llvm_below: self.requires_llvm_below,
            meta_header_skip: self.meta_header_skip,
            gdb: self.gdb,
            lldb: self.lldb,
            ice: self.ice,
        })
    }
}

/// The architectures `requires: target-<arch>` accepts, spelled as
/// `std::env::consts::ARCH` (a typo would otherwise skip the test forever).
pub const TARGET_ARCHES: &[&str] = &["aarch64", "x86_64"];

/// The CPU features `requires: host-<feature>` may name (WP-810).
pub const HOST_FEATURES: &[&str] = &["sve"];

/// The harness host's CPU features (of [`HOST_FEATURES`]): the `Features`
/// line of Linux's `/proc/cpuinfo`, read once; empty elsewhere. `sve` is
/// the kernel's name of the `AArch64` Scalable Vector Extension. The
/// `KILN_HOST_FEATURES` environment variable (space-separated) overrides
/// it, so the skip path is testable.
pub fn host_features() -> &'static [String] {
    static FEATS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    FEATS.get_or_init(|| {
        let text = match std::env::var("KILN_HOST_FEATURES") {
            Ok(v) => v,
            Err(_) => std::fs::read_to_string("/proc/cpuinfo")
                .ok()
                .and_then(|c| {
                    c.lines()
                        .find(|l| l.starts_with("Features") || l.starts_with("flags"))
                        .and_then(|l| l.split_once(':'))
                        .map(|(_, v)| v.to_owned())
                })
                .unwrap_or_default(),
        };
        text.split_whitespace().filter(|f| HOST_FEATURES.contains(f)).map(str::to_owned).collect()
    })
}

/// The architecture of a target triple, in `std::env::consts::ARCH`
/// spelling (clang's `arm64` and `amd64` aliases normalized).
pub fn triple_arch(triple: &str) -> &str {
    match triple.split('-').next().unwrap_or_default() {
        "arm64" => "aarch64",
        "amd64" => "x86_64",
        a => a,
    }
}

fn version_str(v: ClangVersion) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
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
            | "inst-report"
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
            | "cfg-diverges"
            | "requires"
            | "module"
            | "unit"
            | "min-oracle"
            | "meta-header"
            | "gdb"
            | "gdb-check"
            | "lldb"
            | "lldb-check"
            | "ice"
    )
}

/// `<line>[:<col>] <severity> "<substring>" [dup(clang)]`.
fn parse_diag(value: &str, directive_line: usize) -> Result<DiagDirective, String> {
    let usage = "expected `diag: <line>[:<col>] <error|fatal|warning|note|remark> \"<substring>\" [dup(clang)]`";
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
    let rest = rest.trim();
    let (rest, clang_dup) = match rest.strip_suffix("dup(clang)") {
        Some(r) if r.ends_with(char::is_whitespace) => (r.trim_end(), true),
        _ => (rest, false),
    };
    let message = parse_quoted(rest).map_err(|e| format!("{e}; {usage}"))?;
    Ok(DiagDirective { line, col, severity, message, clang_dup, directive_line })
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

    /// `mem<=N` (`spec/harness.md` "Resource limits").
    #[test]
    fn mem_budget() {
        assert_eq!(ok("int x;\n").mem_mib, None);
        assert_eq!(ok("//@ mem<=512\n").mem_mib, Some(512));
        assert_eq!(ok("//@ expect: run\n//@ mem<= 64\n").mem_mib, Some(64));
        let msg =
            "a memory budget is written `mem<=N`, N a positive number of MiB (e.g. `//@ mem<=512`)";
        for bad in [
            "mem: 512",
            "mem<=0",
            "mem<=-1",
            "mem<=1.5",
            "mem<=",
            "mem",
            "mem=512",
            "mem<512",
            "mem<=512: x",
        ] {
            assert_eq!(errs(&format!("//@ {bad}\n")), [format!("line 1: {msg}")], "{bad}");
        }
        assert_eq!(errs("//@ mem<=512\n//@ mem<=256\n"), ["line 2: duplicate `mem<=`"]);
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
    fn no_crash_kind() {
        let s = ok("//@ expect: no-crash\n//@ args: -ferror-limit=0\n#include <string>\n");
        assert_eq!(s.expect, Expect::NoCrash);
        assert_eq!(s.expect.as_str(), "no-crash");
        assert!(!s.oracle);
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
                clang_dup: false,
                directive_line: 3,
            }
        );
        assert_eq!(s.diags[1].severity, Severity::Note);
    }

    #[test]
    fn diag_clang_dup() {
        let s = ok(concat!(
            "//@ expect: compile-fail\n",
            "//@ diag: 2:1 error \"undefined template\" dup(clang)\n",
            "//@ diag: 1:27 note \"declared here\"\n",
            "//@ diag: 3:1 error \"x dup(clang)\"\n",
        ));
        assert!(s.diags[0].clang_dup && !s.diags[1].clang_dup);
        assert_eq!(s.diags[2].message, "x dup(clang)");
        assert!(!s.diags[2].clang_dup);
        assert_eq!(s.diags[0].to_string(), "2:1 error \"undefined template\" dup(clang)");
        let e = parse("//@ diag: 2 error \"a\"\n//@ diag: 3 note \"b\" dup(clang)\n", Lang::Cxx)
            .unwrap_err();
        assert!(format!("{e:?}").contains("put it on the primary"), "{e:?}");
        let e = parse("//@ diag: 2 error \"a\"dup(clang)\n", Lang::Cxx).unwrap_err();
        assert!(format!("{e:?}").contains("unexpected text after message"), "{e:?}");
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
    fn inst_report_directive() {
        let s = ok("//@ inst-report: a.json\n");
        assert_eq!(s.inst_report, Some(PathBuf::from("a.json")));
        assert!(s.is_kiln_specific());
        let s = ok("//@ expect: compile-fail\n//@ inst-report: b.json\n");
        assert_eq!(s.inst_report, Some(PathBuf::from("b.json")));
        assert!(ok("int x;\n").inst_report.is_none());
        let e = errs("//@ expect: run\n//@ inst-report: a.json\n");
        assert!(e.iter().any(|e| e.contains("only applies to `compile-ok`")), "{e:?}");
        let e = errs("//@ inst-report: a.json\n//@ inst-report: b.json\n");
        assert!(e.iter().any(|e| e.contains("duplicate `inst-report`")), "{e:?}");
        let e = errs("//@ inst-report:\n");
        assert!(e.iter().any(|e| e.contains("requires a path")), "{e:?}");
    }

    /// spec/harness.md "Directives" (`verify`): a bare flag, only on
    /// `compile-ok` tests, exclusive with `diag:`/`strict-diags`; the
    /// oracle defaults on and `oracle: none` turns it off.
    #[test]
    fn verify_directive() {
        let s = ok("//@ verify\n");
        assert!(s.verify && s.oracle);
        assert!(!ok("int x;\n").verify);
        let s = ok("//@ verify\n//@ oracle: none\n");
        assert!(s.verify && !s.oracle);
        let e = errs("//@ verify: yes\n");
        assert_eq!(e, ["line 1: `verify` takes no value"]);
        let e = errs("//@ verify\n//@ verify\n");
        assert_eq!(e, ["line 2: duplicate `verify`"]);
        let e = errs("//@ expect: compile-fail\n//@ verify\n");
        assert!(e[0].starts_with("line 2: `verify` only applies to `compile-ok`"), "{e:?}");
        let e = errs("//@ verify\n//@ diag: 3 error \"x\"\n");
        assert!(e[0].starts_with("line 2: `diag:` and `strict-diags` do not apply"), "{e:?}");
        let e = errs("//@ strict-diags\n//@ verify\n");
        assert!(e[0].starts_with("line 1: `diag:` and `strict-diags`"), "{e:?}");
    }

    #[test]
    fn requires_target_directive() {
        let s = ok("//@ expect: run\n//@ requires: target-aarch64\n");
        assert_eq!(s.requires_target.as_deref(), Some("aarch64"));
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None);
        assert_eq!(
            s.unmet_requirement("x86_64", &[], &[]).as_deref(),
            Some("requires target-aarch64; the default target is x86_64")
        );
        assert_eq!(ok("int x;\n").unmet_requirement("x86_64", &[], &[]), None);
        let e = errs("//@ requires: target-aarch46\n");
        assert!(e.iter().any(|e| e.contains("expected `requires: target-<arch>`")), "{e:?}");
        let e = errs("//@ requires: aarch64\n");
        assert!(e.iter().any(|e| e.contains("expected `requires: target-<arch>`")), "{e:?}");
        let e = errs("//@ requires: target-x86_64\n//@ requires: target-x86_64\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: target-`")), "{e:?}");
    }

    #[test]
    fn requires_sanitizer_runtime_directive() {
        let s = ok("//@ expect: run\n//@ requires: sanitizer-runtime\n");
        assert!(s.requires_sanitizer_runtime);
        assert!(!ok("//@ expect: run\n").requires_sanitizer_runtime);
        let e = errs("//@ requires: sanitizer-runtime\n//@ requires: sanitizer-runtime\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: sanitizer-runtime`")), "{e:?}");
    }

    /// WP-810: `requires: host-sve` parses, rejects unknown features and
    /// skips a test that targets another architecture than the host.
    #[test]
    fn requires_host_directive() {
        let s = ok("//@ expect: run\n//@ requires: host-sve\n");
        assert_eq!(s.requires_host.as_deref(), Some("sve"));
        let s = ok("//@ requires: host-sve\n//@ args: --target=x86_64-unknown-linux-gnu\n");
        assert_eq!(
            s.unmet_requirement("aarch64", &[], &[]).as_deref(),
            Some("requires host-sve; the test targets x86_64, not the host")
        );
        let e = errs("//@ requires: host-avx9\n");
        assert!(e.iter().any(|e| e.contains("unknown host feature `avx9`")), "{e:?}");
        let e = errs("//@ requires: host-sve\n//@ requires: host-sve\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: host-`")), "{e:?}");
    }

    /// WP-239: `requires: target-` compares against the test's effective
    /// target, its `--target` when `args:` pin one, not the host.
    #[test]
    fn requires_target_uses_the_pinned_target() {
        let s = ok("//@ requires: target-x86_64\n//@ args: --target=x86_64-unknown-linux-gnu\n");
        assert_eq!(s.target_triple(), Some("x86_64-unknown-linux-gnu"));
        assert_eq!(s.target_arch("aarch64"), "x86_64");
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None);
        let s = ok("//@ requires: target-aarch64\n//@ args: --target=x86_64-unknown-linux-gnu\n");
        assert_eq!(
            s.unmet_requirement("aarch64", &[], &[]).as_deref(),
            Some("requires target-aarch64; the test targets x86_64 (`x86_64-unknown-linux-gnu`)")
        );
        // The last spelling wins, in each of clang's three forms, and the
        // `arm64`/`amd64` aliases are normalized.
        let s = ok("//@ args: --target=x86_64-linux-gnu -target arm64-apple-macos\n");
        assert_eq!(s.target_arch("x86_64"), "aarch64");
        let s = ok("//@ args: -target aarch64-linux-gnu --target amd64-unknown-freebsd\n");
        assert_eq!(s.target_arch("aarch64"), "x86_64");
        let s = ok("//@ args: -O2\n");
        assert_eq!((s.target_triple(), s.target_arch("aarch64")), (None, "aarch64"));
        // A trailing `--target` with no value leaves the default.
        assert_eq!(ok("//@ args: --target\n").target_arch("x86_64"), "x86_64");
    }

    /// WP-239: `requires: clang>=X[.Y[.Z]]` against the clangs a test runs.
    #[test]
    fn requires_clang_directive() {
        let s = ok("//@ requires: clang>=19.1.3\n//@ requires: target-aarch64\n");
        assert_eq!(s.requires_clang, Some([19, 1, 3]));
        assert_eq!(s.requires_target.as_deref(), Some("aarch64"));
        assert_eq!(ok("//@ requires: clang>=20\n").requires_clang, Some([20, 0, 0]));
        let old = ClangRun { role: "the oracle", version: Some([19, 1, 1]) };
        let new = ClangRun { role: "the compiler", version: Some([19, 1, 7]) };
        let unknown = ClangRun { role: "the compiler", version: None };
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None, "kiln alone runs no clang");
        assert_eq!(s.unmet_requirement("aarch64", &[new], &[]), None);
        assert_eq!(
            s.unmet_requirement("aarch64", &[new, old], &[]).as_deref(),
            Some("requires clang>=19.1.3; the oracle is clang 19.1.1")
        );
        assert_eq!(
            s.unmet_requirement("aarch64", &[unknown], &[]).as_deref(),
            Some("requires clang>=19.1.3; the compiler names no clang version")
        );
        // The target requirement is reported first.
        assert_eq!(
            s.unmet_requirement("x86_64", &[old], &[]).as_deref(),
            Some("requires target-aarch64; the default target is x86_64")
        );
        for bad in ["clang>=", "clang>=19.x", "clang>=19.1.3.4", "clang>=.1", "clang>19", "clang"] {
            let e = errs(&format!("//@ requires: {bad}\n"));
            assert!(e.iter().any(|e| e.contains("`requires: clang>=X[.Y[.Z]]`")), "{bad}: {e:?}");
        }
        let e = errs("//@ requires: clang>=19\n//@ requires: clang>=20\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: clang>=`")), "{e:?}");
        assert_eq!(parse_version("19.1.7"), Some([19, 1, 7]));
        assert_eq!(parse_version("19."), None);
    }

    /// WP-355: `requires: clang<X` (an expectation a later clang changed).
    #[test]
    fn requires_clang_below_directive() {
        let s = ok("//@ requires: clang<22\n");
        assert_eq!(s.requires_clang_below, Some([22, 0, 0]));
        let c19 = ClangRun { role: "the compiler", version: Some([19, 1, 7]) };
        let o22 = ClangRun { role: "the oracle", version: Some([22, 1, 8]) };
        let unknown = ClangRun { role: "the oracle", version: None };
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None, "kiln alone runs no clang");
        assert_eq!(s.unmet_requirement("aarch64", &[c19], &[]), None);
        assert_eq!(
            s.unmet_requirement("aarch64", &[c19, o22], &[]).as_deref(),
            Some("requires clang<22.0.0; the oracle is clang 22.1.8")
        );
        assert_eq!(
            s.unmet_requirement("aarch64", &[unknown], &[]).as_deref(),
            Some("requires clang<22.0.0; the oracle names no clang version")
        );
        // Both bounds: the lower one is reported first.
        let both = ok("//@ requires: clang>=19.1.3\n//@ requires: clang<22\n");
        let old = ClangRun { role: "the compiler", version: Some([19, 1, 1]) };
        assert_eq!(
            both.unmet_requirement("aarch64", &[old], &[]).as_deref(),
            Some("requires clang>=19.1.3; the compiler is clang 19.1.1")
        );
        assert_eq!(both.unmet_requirement("aarch64", &[c19], &[]), None);
        for bad in ["clang<", "clang<22.x", "clang<=22"] {
            let e = errs(&format!("//@ requires: {bad}\n"));
            assert!(e.iter().any(|e| e.contains("`requires: clang<X[.Y[.Z]]`")), "{bad}: {e:?}");
        }
        let e = errs("//@ requires: clang<22\n//@ requires: clang<23\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: clang<`")), "{e:?}");
    }

    /// WP-405: `module:` and `min-oracle:`, as `ir_vs_clang` reads them.
    #[test]
    fn module_and_min_oracle_directives() {
        let s =
            ok("//@ module: a=Inputs/a.cppm\n//@ module: b = Inputs/b.cppm\n//@ min-oracle: 22\n");
        let names: Vec<_> =
            s.modules.iter().map(|(n, p)| (n.as_str(), p.to_str().unwrap())).collect();
        assert_eq!(names, [("a", "Inputs/a.cppm"), ("b", "Inputs/b.cppm")]);
        assert_eq!(s.min_oracle, Some(22));
        let c19 = ClangRun { role: "the compiler", version: Some([19, 1, 7]) };
        let c22 = ClangRun { role: "the compiler", version: Some([22, 1, 8]) };
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None, "kiln alone runs no clang");
        assert_eq!(s.unmet_requirement("aarch64", &[c22], &[]), None);
        assert_eq!(
            s.unmet_requirement("aarch64", &[c19], &[]).as_deref(),
            Some("min-oracle 22; the compiler is clang 19.1.7")
        );
        let e = errs("//@ module: a=x.cppm\n//@ module: a=y.cppm\n");
        assert!(e.iter().any(|e| e.contains("duplicate `module` `a`")), "{e:?}");
        for bad in ["a", "=x.cppm", "a=", "a=/abs.cppm"] {
            assert!(!errs(&format!("//@ module: {bad}\n")).is_empty(), "{bad}");
        }
        let s = ok("//@ expect: run\n//@ module: a=x.cppm\n//@ unit: u.cpp\n");
        assert_eq!(s.units, [PathBuf::from("u.cpp")]);
        let e = errs("//@ unit: u.cpp\n");
        assert!(e.iter().any(|e| e.contains("`unit` only applies")), "{e:?}");
        let e = errs("//@ expect: run\n//@ module: a=x.cppm\n//@ two-step\n");
        assert!(e.iter().any(|e| e.contains("`two-step` needs")), "{e:?}");
        let e = errs("//@ two-step\n");
        assert!(e.iter().any(|e| e.contains("`two-step` needs")), "{e:?}");
        let s = ok("//@ module: a=x.cppm\n//@ two-step\n");
        assert!(s.two_step);
        let e = errs("//@ verify\n//@ module: a=x.cppm\n");
        assert!(e.iter().any(|e| e.contains("`module` only applies")), "{e:?}");
        for bad in ["0", "x", "22.1"] {
            let e = errs(&format!("//@ min-oracle: {bad}\n"));
            assert!(e.iter().any(|e| e.contains("`min-oracle` must be")), "{bad}: {e:?}");
        }
        let e = errs("//@ min-oracle: 22\n//@ min-oracle: 23\n");
        assert!(e.iter().any(|e| e.contains("duplicate `min-oracle`")), "{e:?}");
    }

    /// WP-386: `requires: llvm>=N` / `llvm<N` against the backends that
    /// consume the test's IR.
    #[test]
    fn requires_llvm_directive() {
        let s = ok("//@ requires: llvm>=22\n//@ requires: clang>=22\n");
        assert_eq!((s.requires_llvm, s.requires_llvm_below), (Some(22), None));
        let k19 = LlvmRun { role: "kiln's backend", major: Some(19) };
        let k22 = LlvmRun { role: "kiln's backend", major: Some(22) };
        let unknown = LlvmRun { role: "the oracle", major: None };
        assert_eq!(s.unmet_requirement("aarch64", &[], &[]), None, "no backend runs");
        assert_eq!(s.unmet_requirement("aarch64", &[], &[k22]), None);
        assert_eq!(
            s.unmet_requirement("aarch64", &[], &[k22, k19]).as_deref(),
            Some("requires llvm>=22; kiln's backend is LLVM 19")
        );
        assert_eq!(
            s.unmet_requirement("aarch64", &[], &[unknown]).as_deref(),
            Some("requires llvm>=22; the oracle has no known LLVM")
        );
        // The clang requirement is reported first.
        let c19 = ClangRun { role: "the oracle", version: Some([19, 1, 7]) };
        assert_eq!(
            s.unmet_requirement("aarch64", &[c19], &[k19]).as_deref(),
            Some("requires clang>=22.0.0; the oracle is clang 19.1.7")
        );
        let b = ok("//@ requires: llvm<22\n");
        assert_eq!(b.requires_llvm_below, Some(22));
        assert_eq!(b.unmet_requirement("aarch64", &[], &[k19]), None);
        assert_eq!(
            b.unmet_requirement("aarch64", &[], &[k22]).as_deref(),
            Some("requires llvm<22; kiln's backend is LLVM 22")
        );
        for bad in ["llvm>=", "llvm>=22.1", "llvm<x", "llvm>22", "llvm<=22", "llvm>=+2"] {
            let e = errs(&format!("//@ requires: {bad}\n"));
            assert!(e.iter().any(|e| e.contains("`requires: llvm>=N`")), "{bad}: {e:?}");
        }
        let e = errs("//@ requires: llvm>=19\n//@ requires: llvm>=22\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: llvm>=`")), "{e:?}");
        let e = errs("//@ requires: llvm<19\n//@ requires: llvm<22\n");
        assert!(e.iter().any(|e| e.contains("duplicate `requires: llvm<`")), "{e:?}");
    }

    #[test]
    fn skip_clang_implies_oracle_none() {
        // spec/harness.md, directives: a test clang cannot compile is not
        // compared with the (clang) oracle.
        for expect in ["run", "pp"] {
            let s = ok(&format!("//@ expect: {expect}\n//@ skip(clang): hangs\n"));
            assert!(!s.oracle, "skip(clang) implies oracle: none for `{expect}`");
            let s = ok(&format!("//@ expect: {expect}\n//@ skip(kiln): not yet\n"));
            assert!(s.oracle, "skip(kiln) keeps the oracle for `{expect}`");
            let s = ok(&format!("//@ expect: {expect}\n//@ skip(clang): x\n//@ oracle: none\n"));
            assert!(!s.oracle, "an explicit oracle: none is redundant, not an error");
        }
        let s = ok("//@ verify\n//@ skip(clang): hangs\n");
        assert!(!s.oracle, "skip(clang) implies oracle: none for `verify`");
        let e = errs("//@ expect: run\n//@ oracle: clang\n//@ skip(clang): hangs\n");
        assert_eq!(e, ["line 2: `oracle: clang` contradicts `skip(clang)`"]);
        let e = errs("//@ expect: ast\n//@ skip(clang): hangs\n//@ oracle: clang\n");
        assert_eq!(e, ["line 3: `oracle: clang` contradicts `skip(clang)`"]);
    }

    #[test]
    fn gcc_oracle_and_skip_gcc() {
        // spec/harness.md "GCC oracle": `oracle: gcc` replaces the clang
        // oracle on run/compile-ok/compile-fail tests.
        for expect in ["run", "compile-ok", "compile-fail"] {
            let s =
                ok(&format!("//@ expect: {expect}\n//@ oracle: gcc\n//@ skip(clang): no P2996\n"));
            assert!(s.gcc && !s.oracle, "{expect}");
            let s = ok(&format!("//@ expect: {expect}\n"));
            assert!(!s.gcc, "the GCC oracle is opt-in ({expect})");
        }
        let s = ok("//@ expect: compile-ok\n//@ skip(gcc): GCC 16.1 rejects it\n");
        assert_eq!(s.skip_reason(CompilerKind::Gcc), Some("GCC 16.1 rejects it"));
        assert_eq!(s.skip_reason(CompilerKind::Kiln), None);
        let s = ok("//@ xfail(gcc): wrong\n");
        assert_eq!(s.xfail_reason(CompilerKind::Gcc), Some("wrong"));
        let e = errs("//@ oracle: gcc\n//@ skip(gcc): x\n");
        assert_eq!(e, ["line 1: `oracle: gcc` contradicts `skip(gcc)`"]);
        let e = errs("//@ expect: pp\n//@ oracle: gcc\n");
        assert_eq!(
            e,
            ["line 2: `oracle: gcc` only applies to `run`, `compile-ok` and `compile-fail` tests \
              (not `verify`)"]
        );
        let e = errs("//@ verify\n//@ oracle: gcc\n");
        assert!(e[0].contains("(not `verify`)"), "{e:?}");
        let e = errs("//@ oracle: gcc\n//@ oracle: clang\n");
        assert_eq!(e, ["line 2: duplicate `oracle`"]);
    }

    #[test]
    fn skip_clang_rejected_without_golden() {
        // spec/harness.md, directives: `layout`, `vtables` and `ir` compare
        // with the oracle only, so `skip(clang)` would leave no reference.
        for expect in ["layout", "vtables", "ir"] {
            let e = errs(&format!("//@ expect: {expect}\n//@ skip(clang): hangs\n"));
            assert_eq!(
                e,
                ["line 2: `skip(clang)` does not apply to `layout`, `vtables` and `ir` tests \
                     (the oracle is their only reference); use `skip:`"],
                "{expect}"
            );
            let s = ok(&format!("//@ expect: {expect}\n//@ skip(kiln): not yet\n"));
            assert!(s.oracle, "skip(kiln) keeps the oracle for `{expect}`");
            let s = ok(&format!("//@ expect: {expect}\n//@ skip: both\n"));
            assert!(s.oracle && s.skip.len() == 1, "{expect}");
        }
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
            [OvenBudget {
                at: None,
                limits: vec![(Counter::Steps, 100), (Counter::Bytes, 4096)],
                floors: vec![]
            }]
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
            "//@ skip(icc): x\n",
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
        assert!(e[10].contains("unknown compiler `icc` (kiln|clang|gcc)"));
        assert!(e[11].contains("does not take a compiler qualifier"));
        assert!(e[12].contains("unterminated"));
        // exit is also run-only, reported at finish.
        assert!(e[13].contains("`exit` only applies to `expect: run`"));
    }

    #[test]
    fn debugger_directives() {
        let src = "//@ expect: debugger\n//@ gdb: break t.cpp:4\n//@ gdb: run\n\
                   //@ gdb-check: t.cpp:4\n//@ lldb: b t.cpp:4\n//@ lldb-check: stop reason\n";
        let s = parse(src, Lang::Cxx).unwrap();
        assert_eq!(s.expect, Expect::Debugger);
        assert_eq!(s.gdb.commands, ["break t.cpp:4", "run"]);
        assert_eq!(s.gdb.checks, ["t.cpp:4"]);
        assert_eq!(s.lldb.commands, ["b t.cpp:4"]);
        assert!(!s.oracle);
        let e = errs("//@ gdb: run\n");
        assert_eq!(e, ["line 1: `gdb` only applies to `expect: debugger`"]);
        let e = errs("//@ expect: debugger\n");
        assert_eq!(e, ["line 1: `expect: debugger` requires `gdb:` or `lldb:` commands"]);
        let e = errs("//@ expect: debugger\n//@ gdb: run\n//@ lldb-check: x\n");
        assert_eq!(
            e,
            [
                "line 2: `gdb:` commands without a `gdb-check`",
                "line 2: `lldb-check` without `lldb:` commands"
            ]
        );
    }

    #[test]
    fn ice_directives() {
        let src = "//@ expect: ice\n//@ args: -fkiln-crash-for-testing=parse\n\
                   //@ ice: internal compiler error\n//@ ice: phase: parse\n";
        let s = parse(src, Lang::Cxx).unwrap();
        assert_eq!(s.expect, Expect::Ice);
        assert_eq!(s.expect.as_str(), "ice");
        assert_eq!(s.ice, ["internal compiler error", "phase: parse"]);
        assert!(!s.oracle);
        assert_eq!(errs("//@ ice: x\n"), ["line 1: `ice` only applies to `expect: ice`"]);
        assert_eq!(
            errs("//@ expect: ice\n//@ diag: 1 error \"x\"\n"),
            ["line 2: `diag` does not apply to `ice` tests (use `ice:`)"]
        );
        assert_eq!(errs("//@ expect: ice\n//@ ice:\n"), ["line 2: `ice` requires a value"]);
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
        assert_eq!(e, ["line 1: `oracle` only applies to `run`, `pp`, `ast` and `verify` tests"]);
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
        let s = ok("//@ expect: ast\n//@ ast-implicit\n");
        assert!(s.ast_implicit);
        assert!(!ok("//@ expect: ast\n").ast_implicit);
        assert_eq!(
            errs("//@ ast-implicit\n"),
            ["line 1: `ast-implicit` only applies to `expect: ast`"]
        );
        assert_eq!(
            errs("//@ expect: ast\n//@ ast-implicit\n//@ ast-implicit\n"),
            ["line 3: duplicate `ast-implicit`"]
        );
        assert_eq!(
            errs("//@ expect: ast\n//@ ast-implicit: yes\n"),
            ["line 2: `ast-implicit` takes no value"]
        );
        let e = errs("//@ expect: ast\n//@ ast-filter: a\n//@ ast-filter: b\n");
        assert_eq!(e, ["line 3: duplicate `ast-filter`"]);
        let e = errs("//@ expect: ast\n//@ ast-filter: c d\n");
        assert!(e[0].contains("without whitespace"), "{e:?}");
        assert!(errs("//@ ast-filter\n")[0].contains("requires a value"));
    }

    #[test]
    fn layout_and_vtables_directives() {
        for (text, want) in
            [("layout", Expect::Layout), ("vtables", Expect::Vtables), ("ir", Expect::Ir)]
        {
            let s = ok(&format!("//@ expect: {text}\n//@ diag: 2 warning \"w\"\n"));
            assert_eq!(s.expect, want);
            assert_eq!(s.expect.as_str(), text);
            assert!(s.oracle, "the oracle is the only reference of `{text}`");
            let e = errs(&format!("//@ expect: {text}\n//@ oracle: none\n"));
            assert_eq!(
                e,
                ["line 2: `oracle` only applies to `run`, `pp`, `ast` and `verify` tests"]
            );
            let e = errs(&format!("//@ expect: {text}\n//@ ast-filter: f\n"));
            assert_eq!(e, ["line 2: `ast-filter` only applies to `expect: ast`"]);
        }
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
