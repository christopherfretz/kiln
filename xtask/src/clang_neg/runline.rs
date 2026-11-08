//! Turning a clang test's `// RUN:` lines into kiln invocations
//! (spec/clang-neg.md "RUN lines"): which lines are `%clang_cc1 ...
//! -verify` runs, how their cc1 flags map to kiln++ driver flags, and why
//! the others are skipped. Pure text processing, no I/O.

/// The standards a `%std_cxx<from>-<to>` substitution walks, in order
/// (lit's `std_cxx` expansion; an open upper end runs to the newest).
pub const STD_STEPS: &[&str] = &["98", "11", "14", "17", "20", "23", "26"];

/// The `REQUIRES:` features kiln's runs satisfy: an x86-64 or AArch64
/// Linux target, LP64, a POSIX shell and threads.
pub const FEATURES: &[&str] = &[
    "x86-registered-target",
    "aarch64-registered-target",
    "LP64",
    "system-linux",
    "shell",
    "thread_support",
];

/// cc1 flag prefixes whose runs are skipped: language modes and
/// extensions kiln does not implement, other languages, multi-step
/// (module/PCH) runs and non-syntax actions (spec/clang-neg.md "Skips").
pub const SKIP_FLAGS: &[&str] = &[
    "-fms-",
    "-fobjc",
    "-fopenmp",
    "-fcuda",
    "-fhip",
    "-fsycl",
    "-fopencl",
    "-cl-",
    "-fhlsl",
    "-fmodule",
    "-fmodules",
    "-fprebuilt-module",
    "-fimplicit-module",
    "-emit-module",
    "-emit-pch",
    "-include-pch",
    "-fblocks",
    "-fdelayed-template-parsing",
    "-fborland-extensions",
    "-fdeclspec",
    "-fptrauth",
    "-fenable-matrix",
    "-fzvector",
    "-fzos",
    "-fasm-blocks",
    "-fpascal-strings",
    "-fclang-abi-compat",
    "-aux-triple",
    "-ast-",
    "-analyze",
    "-fsanitize",
    "-E",
    "-fapple-kext",
    "-fgpu",
    "-fnative-half",
    "-fexperimental-new-constant-interpreter",
];

/// cc1 flags without effect on whether an error is reported (output
/// format, fix-its, spelling-correction limits): dropped. A trailing `=`
/// or `*` entry matches any value; `+` entries take the next token.
const DROP_FLAGS: &[&str] = &[
    "-fsyntax-only",
    "-fno-caret-diagnostics",
    "-fcaret-diagnostics",
    "-fcolor-diagnostics",
    "-fno-color-diagnostics",
    "-fdiagnostics-*",
    "-fno-diagnostics-*",
    "-fshow-overloads=*",
    "-fshow-column",
    "-fno-show-column",
    "-fshow-source-location",
    "-fno-show-source-location",
    "-fspell-checking-limit*",
    "-fno-spell-checking",
    "-fmacro-backtrace-limit*",
    "-fixit",
    "-fixit=*",
    "-ferror-limit=*",
    "-ferror-limit+",
    "-verify-ignore-unexpected",
    "-verify-ignore-unexpected=*",
    "-emit-llvm",
    "-emit-llvm-only",
    "-emit-obj",
    "-S",
    "-o+",
    "-fexceptions",
    "-fno-exceptions",
    "-fno-cxx-exceptions",
    "-fdump-record-layouts",
    "-fdump-record-layouts-simple",
];

/// One kiln invocation derived from a RUN line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaseCfg {
    /// 1-based index of the RUN command in the file.
    pub run: usize,
    /// The `-std=` value kiln gets.
    pub std: String,
    /// The `-verify` prefixes, in order, deduplicated (`expected` for a
    /// bare `-verify`).
    pub prefixes: Vec<String>,
    /// kiln++ arguments other than `-fsyntax-only`, `-verify=` and the
    /// input, in order.
    pub args: Vec<String>,
}

impl CaseCfg {
    /// A short label: `RUN 2 -std=c++11 -verify=expected,cxx11`.
    pub fn label(&self) -> String {
        format!("RUN {} -std={} -verify={}", self.run, self.std, self.prefixes.join(","))
    }
}

/// What one RUN command contributes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RunPlan {
    /// One case per standard (more than one for `%std_cxx<a>-<b>`).
    Cases(Vec<CaseCfg>),
    /// A `-verify` run kiln does not attempt, with the reason; counts once
    /// per configuration it stands for.
    Skip { reason: String, configs: usize },
    /// Not a `-verify` run of `%clang_cc1` (FileCheck, `split-file`, ...).
    NotVerify,
}

/// The RUN commands of `text`, in order: the text after `RUN:` with
/// trailing-`\` continuation lines joined.
pub fn run_commands(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending: Option<String> = None;
    for line in text.lines() {
        let Some(i) = line.find("RUN:") else {
            if let Some(p) = pending.take() {
                out.push(p);
            }
            continue;
        };
        let body = line[i + 4..].trim();
        let (body, cont) = match body.strip_suffix('\\') {
            Some(b) => (b.trim_end(), true),
            None => (body, false),
        };
        let mut cur = pending.take().unwrap_or_default();
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(body);
        if cont {
            pending = Some(cur);
        } else {
            out.push(cur);
        }
    }
    out.extend(pending);
    out
}

/// Splits a command on whitespace, honouring `'...'` and `"..."`.
pub fn tokenize(cmd: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut any = false;
    let mut quote: Option<char> = None;
    for c in cmd.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => cur.push(c),
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                any = true;
            }
            None if c.is_whitespace() => {
                if any || !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                any = false;
            }
            None => cur.push(c),
        }
    }
    if any || !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The standards of `%std_cxx<from>-<to>` (`to` may be empty: open).
fn std_range(sub: &str) -> Option<Vec<String>> {
    let range = sub.strip_prefix("%std_cxx")?;
    let (from, to) = range.split_once('-')?;
    let lo = STD_STEPS.iter().position(|s| *s == from)?;
    let hi = if to.is_empty() { STD_STEPS.len() - 1 } else { STD_STEPS.iter().position(|s| *s == to)? };
    (lo <= hi).then(|| STD_STEPS[lo..=hi].iter().map(|s| format!("c++{s}")).collect())
}

/// Whether `-triple <t>` is one kiln runs: x86-64 or AArch64, Linux or
/// the generic `unknown-unknown` ELF target. `Ok(driver triple)`, else
/// the skip reason.
pub fn map_triple(t: &str) -> Result<String, String> {
    // lit's `%itanium_abi_triple` is the host triple on a non-Windows
    // host: kiln's default target. An empty result means no `--target=`.
    if t == "%itanium_abi_triple" {
        return Ok(String::new());
    }
    let mut parts = t.split('-');
    let arch = match parts.next().unwrap_or("") {
        "arm64" => "aarch64",
        a => a,
    };
    let rest: Vec<&str> = parts.collect();
    let arch_ok = matches!(arch, "x86_64" | "aarch64");
    if !arch_ok {
        return Err(format!("-triple {arch}-*"));
    }
    let linux = rest.iter().any(|p| p.starts_with("linux"));
    let generic = rest.is_empty() || rest.iter().all(|p| matches!(*p, "unknown" | "pc" | "none" | "elf"));
    if linux {
        Ok(std::iter::once(arch).chain(rest.iter().copied()).collect::<Vec<_>>().join("-"))
    } else if generic {
        Ok(format!("{arch}-unknown-linux-gnu"))
    } else {
        let os = rest.iter().find(|p| !matches!(**p, "unknown" | "pc")).copied().unwrap_or("?");
        Err(format!("-triple *-{os}"))
    }
}

/// `Some(takes_value)` when `tok` is in `table` (see [`DROP_FLAGS`]).
fn flag_matches(table: &[&str], tok: &str) -> Option<bool> {
    table.iter().find_map(|f| {
        let hit = if let Some(p) = f.strip_suffix('*') {
            tok.starts_with(p)
        } else if let Some(p) = f.strip_suffix('+') {
            return (tok == p).then_some(true);
        } else if let Some(p) = f.strip_suffix('=') {
            tok.strip_prefix(p).is_some_and(|r| r.starts_with('='))
        } else {
            tok == *f
        };
        hit.then_some(false)
    })
}

/// Plans the RUN command `cmd` (the `run`-th of the file). `dir` is the
/// test's directory (`%S`).
#[allow(clippy::too_many_lines)] // one flat flag table walk
pub fn plan(cmd: &str, run: usize, dir: &str) -> RunPlan {
    let toks = tokenize(cmd);
    if !toks.iter().any(|t| t == "-verify" || t.starts_with("-verify=")) {
        return RunPlan::NotVerify;
    }
    let mut it = toks.iter().map(String::as_str).peekable();
    let mut head = it.next().unwrap_or("");
    if head == "not" {
        if it.peek() == Some(&"--crash") {
            return skip("not --crash", 1);
        }
        head = it.next().unwrap_or("");
    }
    if head != "%clang_cc1" {
        let what = if head.starts_with("%clang") { "driver RUN (%clang)" } else { "RUN is not %clang_cc1" };
        return skip(what, 1);
    }
    let mut stds: Option<Vec<String>> = None;
    let mut prefixes: Vec<String> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    let mut exceptions = false;
    let mut triple: Option<String> = None;
    let mut saw_input = false;
    let mut pending_skip: Option<String> = None;
    while let Some(tok) = it.next() {
        if matches!(tok, "|" | "||" | "&&" | ";") || tok.starts_with('>') || tok.contains(">&") || tok.starts_with("2>") {
            return skip("pipe, redirection or compound RUN", stds.as_ref().map_or(1, Vec::len));
        }
        if tok.contains("%t") || tok.contains("%T") || tok.contains("%{") {
            return skip("uses %t or %{...} (multi-step RUN)", 1);
        }
        if let Some(r) = std_range(tok) {
            stds = Some(r);
            continue;
        }
        if tok.starts_with("%std_cxx") {
            return skip("unknown %std_cxx substitution", 1);
        }
        if tok == "%s" {
            saw_input = true;
            continue;
        }
        if tok == "-verify" {
            prefixes.push("expected".to_owned());
            continue;
        }
        if let Some(v) = tok.strip_prefix("-verify=") {
            prefixes.extend(v.split(',').filter(|p| !p.is_empty()).map(str::to_owned));
            continue;
        }
        if let Some(v) = tok.strip_prefix("-std=").or_else(|| tok.strip_prefix("--std=")) {
            stds = Some(vec![v.to_owned()]);
            continue;
        }
        if tok == "-triple" || tok.starts_with("-triple=") || tok == "-target" {
            let t = match tok.strip_prefix("-triple=") {
                Some(v) => v,
                None => it.next().unwrap_or(""),
            };
            match map_triple(t) {
                Ok(d) => triple = Some(d),
                Err(r) => pending_skip = pending_skip.or(Some(r)),
            }
            continue;
        }
        if tok == "-x" {
            let lang = it.next().unwrap_or("");
            if lang != "c++" {
                pending_skip = pending_skip.or(Some(format!("-x {lang}")));
            }
            continue;
        }
        if tok == "-fcxx-exceptions" {
            exceptions = true;
            continue;
        }
        if SKIP_FLAGS.iter().any(|p| tok == *p || (p.len() > 2 && tok.starts_with(p))) {
            let shown = tok.split('=').next().unwrap_or(tok);
            pending_skip = pending_skip.or(Some(shown.to_owned()));
            continue;
        }
        match flag_matches(DROP_FLAGS, tok) {
            Some(true) => {
                it.next();
                continue;
            }
            Some(false) => continue,
            None => {}
        }
        // Flags that take the next token as their value.
        if matches!(tok, "-D" | "-U" | "-I" | "-isystem" | "-iquote" | "-idirafter" | "-include" | "-target-feature" | "-target-cpu") {
            let v = it.next().unwrap_or("");
            let v = v.replace("%S", dir);
            if tok == "-target-cpu" {
                args.push(format!("-mcpu={v}"));
            } else {
                args.push(tok.to_owned());
                args.push(v);
            }
            continue;
        }
        if tok.starts_with('-') {
            args.push(tok.replace("%S", dir));
            continue;
        }
        if tok.starts_with('%') {
            return skip("unknown % substitution", 1);
        }
        return skip("extra input file", 1);
    }
    let stds = stds.unwrap_or_else(|| vec!["gnu++17".to_owned()]);
    if let Some(r) = pending_skip {
        return skip(&r, stds.len());
    }
    if !saw_input {
        return skip("no %s input", stds.len());
    }
    let mut seen = std::collections::BTreeSet::new();
    prefixes.retain(|p| seen.insert(p.clone()));
    let mut base = Vec::new();
    if let Some(t) = triple.filter(|t| !t.is_empty()) {
        base.push(format!("--target={t}"));
    }
    if !exceptions {
        // cc1 enables C++ exceptions only with -fcxx-exceptions; the kiln
        // driver, like clang's, enables them by default.
        base.push("-fno-exceptions".to_owned());
    }
    base.extend(args);
    RunPlan::Cases(
        stds.into_iter()
            .map(|std| CaseCfg { run, std, prefixes: prefixes.clone(), args: base.clone() })
            .collect(),
    )
}

fn skip(reason: &str, configs: usize) -> RunPlan {
    RunPlan::Skip { reason: reason.to_owned(), configs }
}

/// The reason the file's `REQUIRES:`, `UNSUPPORTED:` or `XFAIL:` lines
/// keep kiln from running it, if they do. `REQUIRES` terms are
/// comma-separated (all must hold) and may use `||`, `&&` and `!`;
/// `UNSUPPORTED` skips when any term holds; `XFAIL: *` skips. Features
/// are [`FEATURES`]; a `target=` term holds when it names Linux.
pub fn requirements(text: &str) -> Option<String> {
    for line in text.lines() {
        if let Some(i) = line.find("REQUIRES:") {
            for term in line[i + 9..].split(',') {
                let term = term.trim();
                if !term.is_empty() && !eval(term) {
                    return Some(format!("REQUIRES: {term}"));
                }
            }
        } else if let Some(i) = line.find("UNSUPPORTED:") {
            for term in line[i + 12..].split(',') {
                let term = term.trim();
                if !term.is_empty() && eval(term) {
                    return Some(format!("UNSUPPORTED: {term}"));
                }
            }
        } else if let Some(i) = line.find("XFAIL:") {
            let rest = line[i + 6..].trim();
            if rest.split(',').any(|t| t.trim() == "*" || eval(t.trim())) {
                return Some(format!("XFAIL: {rest}"));
            }
        }
    }
    None
}

/// Evaluates one feature expression (`a || b`, `a && !b`; no parentheses).
fn eval(expr: &str) -> bool {
    expr.split("||").any(|conj| {
        conj.split("&&").all(|atom| {
            let atom = atom.trim().trim_matches(|c| c == '(' || c == ')').trim();
            match atom.strip_prefix('!') {
                Some(a) => !feature(a.trim()),
                None => feature(atom),
            }
        })
    })
}

fn feature(f: &str) -> bool {
    if let Some(t) = f.strip_prefix("target=") {
        return t.contains("linux");
    }
    FEATURES.contains(&f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cases(cmd: &str) -> Vec<CaseCfg> {
        match plan(cmd, 1, "/t") {
            RunPlan::Cases(c) => c,
            other => panic!("{cmd}: {other:?}"),
        }
    }

    fn skipped(cmd: &str) -> String {
        match plan(cmd, 1, "/t") {
            RunPlan::Skip { reason, .. } => reason,
            other => panic!("{cmd}: {other:?}"),
        }
    }

    /// spec/clang-neg.md "RUN lines": continuation lines are joined.
    #[test]
    fn run_lines_are_joined() {
        let t = "// RUN: %clang_cc1 -verify \\\n// RUN:   -std=c++11 %s\nint x;\n// RUN: echo hi\n";
        assert_eq!(run_commands(t), ["%clang_cc1 -verify -std=c++11 %s", "echo hi"]);
    }

    /// spec/clang-neg.md "Flag mapping".
    #[test]
    fn flags_map_to_the_driver() {
        let c = cases("%clang_cc1 -fsyntax-only -verify=expected,cxx11 -std=c++11 -Wno-unused -DX=1 -I %S/Inputs %s");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].std, "c++11");
        assert_eq!(c[0].prefixes, ["expected", "cxx11"]);
        assert_eq!(c[0].args, ["-fno-exceptions", "-Wno-unused", "-DX=1", "-I", "/t/Inputs"]);
        let c = cases("%clang_cc1 -triple x86_64-unknown-unknown -fcxx-exceptions -fexceptions -verify %s");
        assert_eq!(c[0].args, ["--target=x86_64-unknown-linux-gnu"]);
        assert_eq!(c[0].std, "gnu++17");
        let c = cases("not %clang_cc1 -verify -ferror-limit 3 -fno-caret-diagnostics -pedantic-errors %s");
        assert_eq!(c[0].args, ["-fno-exceptions", "-pedantic-errors"]);
    }

    /// spec/clang-neg.md "RUN lines": `%std_cxx<a>-<b>` is one case per
    /// standard.
    #[test]
    fn std_ranges_expand() {
        let s: Vec<String> = cases("%clang_cc1 -verify %std_cxx98-14 %s").into_iter().map(|c| c.std).collect();
        assert_eq!(s, ["c++98", "c++11", "c++14"]);
        let s: Vec<String> = cases("%clang_cc1 -verify %std_cxx20- %s").into_iter().map(|c| c.std).collect();
        assert_eq!(s, ["c++20", "c++23", "c++26"]);
        assert_eq!(plan("%clang_cc1 -fblocks -verify %std_cxx17- %s", 1, "/t"), RunPlan::Skip { reason: "-fblocks".into(), configs: 4 });
    }

    /// spec/clang-neg.md "Skips".
    #[test]
    fn skip_reasons() {
        assert_eq!(plan("%clang_cc1 -emit-llvm %s -o - | FileCheck %s", 1, "/t"), RunPlan::NotVerify);
        assert_eq!(skipped("%clang_cc1 -fms-extensions -verify %s"), "-fms-extensions");
        assert_eq!(skipped("%clang_cc1 -triple i686-linux-gnu -verify %s"), "-triple i686-*");
        assert_eq!(skipped("%clang_cc1 -triple x86_64-apple-darwin -verify %s"), "-triple *-apple");
        assert_eq!(skipped("%clang_cc1 -triple arm64-apple-ios -verify %s"), "-triple *-apple");
        assert_eq!(map_triple("arm64-linux-gnu"), Ok("aarch64-linux-gnu".to_owned()));
        assert_eq!(map_triple("%itanium_abi_triple"), Ok(String::new()));
        assert_eq!(cases("%clang_cc1 -triple %itanium_abi_triple -verify %s")[0].args, ["-fno-exceptions"]);
        assert_eq!(cases("%clang_cc1 -verify --std=c++17 %s")[0].std, "c++17");
        assert_eq!(skipped("%clang_cc1 -triple x86_64-pc-windows-msvc -verify %s"), "-triple *-windows");
        assert_eq!(skipped("%clang_cc1 -verify -fmodules-ts %t/a.cpp"), "uses %t or %{...} (multi-step RUN)");
        assert_eq!(skipped("%clang -fsyntax-only -Xclang -verify %s"), "driver RUN (%clang)");
        assert_eq!(skipped("%clang_cc1 -x objective-c++ -verify %s"), "-x objective-c++");
        assert_eq!(skipped("%clang_cc1 -verify %s 2>&1"), "pipe, redirection or compound RUN");
        assert_eq!(skipped("%clang_cc1 -fexperimental-new-constant-interpreter -verify %s"), "-fexperimental-new-constant-interpreter");
    }

    /// spec/clang-neg.md "Skips": REQUIRES / UNSUPPORTED / XFAIL.
    #[test]
    fn requirement_lines() {
        assert_eq!(requirements("// REQUIRES: x86-registered-target\n"), None);
        assert_eq!(requirements("// REQUIRES: aarch64-registered-target || arm-registered-target\n"), None);
        assert_eq!(requirements("// REQUIRES: asserts\n"), Some("REQUIRES: asserts".into()));
        assert_eq!(requirements("// UNSUPPORTED: system-netbsd\n"), None);
        assert_eq!(requirements("// UNSUPPORTED: target={{.*}}-linux{{.*}}\n").is_some(), true);
        assert_eq!(requirements("// XFAIL: *\n"), Some("XFAIL: *".into()));
    }
}
