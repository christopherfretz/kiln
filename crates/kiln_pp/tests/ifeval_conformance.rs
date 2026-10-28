//! WP-015: the `#if` conformance and torture cases run through `kiln_pp`
//! with [`IfEval`] and [`ClangFeatures`] installed ([cpp.cond]):
//! `tests/conformance/cpp.cond/**` and the `cpp.cond-*`,
//! `cpp.has-feature-macros` and `cpp.defined-in-macro` torture cases of
//! `tests/differential/pp/torture/`. Output is compared token-wise with the
//! checked-in expectation (line markers and blank lines ignored).

mod ifeval_common;

use std::path::{Path, PathBuf};

use ifeval_common::{run_kiln, words};
use kiln_lex::Std;
use kiln_pp::{ClangFeatures, MemoryResolver};

const TRIPLES: [&str; 2] = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(p: &Path) -> String {
    std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// The `--target=` of a case's `//@ args:` line, if any.
fn pinned_target(src: &str) -> Option<String> {
    src.lines()
        .filter_map(|l| l.strip_prefix("//@ args:"))
        .flat_map(str::split_whitespace)
        .find_map(|a| a.strip_prefix("--target=").map(str::to_owned))
}

/// A resolver serving every file of `dir` both as `name` and as
/// `<dir name>/name` (the case's `-I<dir>` plus quoted lookup relative to
/// the case).
fn inputs(dir: &Path) -> MemoryResolver {
    let mut r = MemoryResolver::new();
    let prefix = dir.file_name().unwrap().to_string_lossy().into_owned();
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if let Ok(text) = std::fs::read_to_string(&p) {
                let name = p.file_name().unwrap().to_string_lossy().into_owned();
                r = r.with_header(&name, &text).with_header(&format!("{prefix}/{name}"), &text);
            }
        }
    }
    r
}

/// Output tokens, preprocessor line markers dropped.
fn tokens(text: &str) -> Vec<String> {
    let kept: String =
        text.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    words(&kept).into_iter().map(str::to_owned).collect()
}

/// Runs `case` for each target (or its pinned one) and compares with
/// `expected`; diagnostics are not expected.
fn check(case: &Path, expected: &str) {
    let src = read(case);
    let triples: Vec<String> = match pinned_target(&src) {
        Some(t) => vec![t],
        None => TRIPLES.iter().map(|t| (*t).to_owned()).collect(),
    };
    for triple in triples {
        check_with(case, &triple, expected);
    }
}

/// The case's `//@ diag:` directives as `(line, severity, substring)`,
/// and whether it is `strict-diags`.
fn directives(src: &str) -> (Vec<(String, String, String)>, bool) {
    let mut v = Vec::new();
    let mut strict = false;
    for l in src.lines().filter_map(|l| l.strip_prefix("//@")).map(str::trim) {
        strict |= l == "strict-diags";
        if let Some(d) = l.strip_prefix("diag:") {
            let mut it = d.trim().splitn(3, ' ');
            let (at, sev, msg) = (it.next().unwrap(), it.next().unwrap(), it.next().unwrap());
            v.push((at.to_owned(), sev.to_owned(), msg.trim().trim_matches('"').to_owned()));
        }
    }
    (v, strict)
}

/// [`check`] for one `triple`. Diagnostics follow spec/harness.md: each
/// `diag:` directive matches a distinct diagnostic, and under
/// `strict-diags` nothing else may be reported. Without `-I` arguments
/// (stdin cannot resolve relative includes) the diagnostics must also
/// equal clang's, when clang is installed.
fn check_with(case: &Path, triple: &str, expected: &str) {
    let src = read(case);
    let resolver = inputs(&case.parent().unwrap().join("Inputs"));
    let out = run_kiln(&src, triple, Std::Cxx26, &[], resolver, |pp| {
        let f = ClangFeatures::new(triple, Std::Cxx26, &[]).unwrap();
        pp.set_feature_queries(Box::new(f));
    });
    let what = format!("{} ({triple})", case.display());
    assert_eq!(tokens(&out.text), tokens(expected), "{what}:\n{}", out.text);
    let (dirs, strict) = directives(&src);
    let mut unmatched = out.diags.clone();
    for (at, sev, msg) in &dirs {
        let i = unmatched.iter().position(|d| {
            let (loc, rest) = d.split_once(": ").unwrap_or(("", d));
            let line_ok =
                if at.contains(':') { loc == at } else { loc.split(':').next() == Some(at) };
            line_ok && rest.starts_with(&format!("{sev}: ")) && rest.contains(msg.as_str())
        });
        let i = i.unwrap_or_else(|| {
            panic!("{what}: no diagnostic for {at} {sev} {msg:?}: {:?}", out.diags)
        });
        unmatched.remove(i);
    }
    if strict {
        assert_eq!(unmatched, [] as [String; 0], "{what}: unexpected {unmatched:?}");
    }
    let has_args = src.lines().any(|l| l.starts_with("//@ args:") && l.contains("-I"));
    let clang =
        if has_args { None } else { ifeval_common::run_clang(&src, triple, Std::Cxx26, &[]) };
    if let Some(c) = clang {
        assert_eq!(out.diags, c.diags, "{what}: diagnostics differ from clang");
    }
}

/// [cpp.cond]/8-12: the torture cases, against their checked-in clang
/// `-E -P` output: every `cpp.cond-*` case plus `cpp.has-feature-macros`
/// and `cpp.defined-in-macro`.
#[test]
fn torture_cases() {
    let dir = root().join("tests/differential/pp/torture");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.starts_with("cpp.cond-") && Path::new(n).extension().is_some_and(|e| e == "cpp")
        })
        .collect();
    assert!(names.len() >= 2, "{names:?}");
    names.sort();
    names.push("cpp.has-feature-macros.cpp".to_owned());
    names.push("cpp.defined-in-macro.cpp".to_owned());
    for name in names {
        let expected = read(&dir.join(format!("{name}.expected")));
        check(&dir.join(&name), &expected);
    }
}

/// `tests/conformance/cpp.cond/**` (oracle: clang). Each case's expected
/// output is clang's `-E -P` output for it as C++26 (kiln is a C++ front
/// end; C-mode cases use only `#if` semantics shared with C++). Without
/// clang, the surviving lines are checked against the case's markers: the
/// non-directive lines not ending in `_wrong`.
#[test]
fn conformance_cases() {
    let dir = root().join("tests/conformance/cpp.cond");
    let mut n = 0;
    for e in std::fs::read_dir(&dir).unwrap().flatten() {
        let src = read(&e.path());
        let markers: Vec<&str> = src
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"))
            .filter(|l| !l.trim_end_matches(';').ends_with("_wrong"))
            .collect();
        check(&e.path(), &markers.join("\n"));
        for triple in TRIPLES {
            if let Some(c) = ifeval_common::run_clang(&src, triple, Std::Cxx26, &[]) {
                check_with(&e.path(), triple, &c.text);
            }
        }
        n += 1;
    }
    assert!(n >= 1);
}
