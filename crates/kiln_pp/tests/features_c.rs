//! WP-1275: the feature-test hooks in C (spec/pp.md "Feature queries in
//! C"; C23 6.10.1, 6.10.2 `__has_c_attribute`) against `clang-19 -x c`.
//!
//! Every key of both generated tables is queried through `kiln_pp` with
//! [`ClangFeatures::with_c`] installed and through `clang-19 -x c -E
//! -undef` (and, at `ClangCompat::Clang22`, `clang-22`), in every C
//! standard (strict and GNU) on both targets, and the answers must be
//! identical; every `__has_cpp_attribute` key is also asked of
//! `__has_c_attribute`. The same holds under each flag the generator
//! probes (WP-1290). A clang that is not installed is skipped.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::process::{Command, Stdio};

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{CStd, Interner, Std};
use kiln_pp::{
    ClangCompat, ClangFeatures, FeatureQueries, IfEval, MemoryResolver, NoFeatures, PpOptions,
    Preprocessor, PrintOptions, print_preprocessed,
};
use kiln_source::SourceManager;

const ORACLES: [(&str, ClangCompat); 2] =
    [("clang-19", ClangCompat::Clang19), ("clang-22", ClangCompat::Clang22)];
const TABLES: [&str; 2] =
    [include_str!("../src/features_table.rs"), include_str!("../src/features_c_table.rs")];
const TRIPLES: [&str; 2] = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];
/// The flags `scripts/gen-features-c.sh` probes (`C19_FLAGS`/`C22_FLAGS`).
const FLAGS: [&str; 8] = [
    "-fno-exceptions",
    "-fmodules",
    "-fblocks",
    "-fms-extensions",
    "-fsanitize=address",
    "-fsanitize=thread",
    "-fsanitize=memory",
    "-fsanitize=undefined",
];
const ACLE: [&str; 8] = ["__dmb", "__dsb", "__isb", "__sev", "__sevl", "__wfe", "__wfi", "__yield"];
/// (C standard, clang spelling, the C++ standard it lexes as).
const CSTDS: [(CStd, &str, Std); 5] = [
    (CStd::C89, "89", Std::Cxx98),
    (CStd::C99, "99", Std::Cxx98),
    (CStd::C11, "11", Std::Cxx11),
    (CStd::C17, "17", Std::Cxx11),
    (CStd::C23, "23", Std::Cxx17),
];

fn hook(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "FEATURE" | "Feature" => "__has_feature",
        "EXTENSION" | "Extension" => "__has_extension",
        "BUILTIN" | "Builtin" => "__has_builtin",
        "CONSTEXPR_BUILTIN" | "ConstexprBuiltin" => "__has_constexpr_builtin",
        "ATTRIBUTE" | "Attribute" => "__has_attribute",
        "DECLSPEC_ATTRIBUTE" | "DeclspecAttribute" => "__has_declspec_attribute",
        "CPP_ATTRIBUTE" | "CppAttribute" | "CAttribute" => "__has_c_attribute",
        "KEYWORD" | "Keyword" => "__is_identifier",
        _ => return None,
    })
}

/// Every key of both tables as a C query, plus the hook-existence probes.
fn queries() -> Vec<String> {
    let mut out = Vec::new();
    for table in TABLES {
        let mut section = "";
        for line in table.lines() {
            if let Some(rest) = line.strip_prefix("pub(super) static ") {
                section = rest.split(':').next().unwrap();
                continue;
            }
            let t = line.trim_start();
            let kind = if t.starts_with("Entry(") {
                section
            } else if let Some(k) = t.split_once("Kind::").map(|(_, k)| k) {
                k.split(',').next().unwrap()
            } else {
                continue;
            };
            let (Some(h), Some(a)) = (hook(kind), t.find('"')) else { continue };
            let key = &t[a + 1..a + 1 + t[a + 1..].find('"').unwrap()];
            out.push(format!("{h}({key})"));
        }
    }
    out.extend(
        ["__nodiscard__", "__gnu__::__unused__", "_Clang::overloadable", "gnu::__unused__"]
            .map(|k| format!("__has_c_attribute({k})")),
    );
    out.extend(["defined(__has_cpp_attribute)", "defined(__has_c_attribute)"].map(String::from));
    out.sort();
    out.dedup();
    out
}

fn source(qs: &[String]) -> String {
    let mut src = String::new();
    for (i, q) in qs.iter().enumerate() {
        if let Some(m) = q.strip_prefix("defined(") {
            let m = m.trim_end_matches(')');
            writeln!(src, "#ifdef {m}\nL{i} 1\n#else\nL{i} 0\n#endif").unwrap();
        } else {
            writeln!(src, "L{i} {q}").unwrap();
        }
    }
    src
}

/// `L<i> value` lines -> {i: value}.
fn values(text: &str) -> BTreeMap<usize, String> {
    let mut m = BTreeMap::new();
    for l in text.lines() {
        let mut it = l.split_whitespace();
        if let (Some(a), Some(v)) = (it.next(), it.next())
            && let Some(i) = a.strip_prefix('L').and_then(|i| i.parse().ok())
        {
            m.insert(i, v.to_owned());
        }
    }
    m
}

fn clang(cc: &str, src: &str, triple: &str, std: &str, flag: Option<&str>) -> Option<String> {
    let mut cmd = Command::new(cc);
    if let Some(f) = flag {
        cmd.arg(f);
        if f == "-fmodules" {
            cmd.arg(format!("-fmodules-cache-path={}/wp1290-mc", env!("CARGO_TARGET_TMPDIR")));
        }
    }
    let mut child = cmd
        .args(["-x", "c", "-E", "-P", "-undef", "-nostdinc", "-ferror-limit=0"])
        .arg(format!("-std={std}"))
        .arg(format!("--target={triple}"))
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdin = child.stdin.take()?;
    let input = src.to_owned();
    let writer = std::thread::spawn(move || stdin.write_all(input.as_bytes()).is_ok());
    let out = child.wait_with_output().ok()?;
    writer.join().ok()?.then_some(())?;
    String::from_utf8(out.stdout).ok()
}

fn kiln(
    src: &str,
    features: Box<dyn FeatureQueries>,
    triple: &str,
    c: CStd,
    lex: Std,
    gnu: bool,
) -> String {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.c", src.to_owned()).unwrap();
    let mut dopts = DiagOptions::from_args(std::iter::empty::<&str>());
    dopts.parse_arg("-ferror-limit=0");
    let engine = DiagEngine::new(&sm, &dopts, MemorySink::new());
    let mut opts = PpOptions::new(lex);
    opts.c = Some(c);
    opts.gnu = gnu;
    let mut pp = Preprocessor::new(&sm, &engine, &interner, opts);
    pp.set_include_resolver(Box::new(MemoryResolver::new()));
    pp.set_if_evaluator(Box::new(IfEval::for_triple(triple)));
    pp.set_feature_queries(features);
    pp.enter_main_file(main);
    let mut text = String::new();
    print_preprocessed(&mut pp, PrintOptions::default(), &mut text);
    text
}

#[test]
fn c_answers_match_clang_in_every_configuration() {
    check(None);
}

/// Under each probed flag: the C++ overlays, the C deltas, then the flag's
/// C deltas (WP-1290).
#[test]
fn c_answers_match_clang_under_flags() {
    for f in FLAGS {
        check(Some(f));
    }
}

fn check(flag: Option<&str>) {
    let qs = queries();
    assert!(qs.len() > 3000, "only {} queries", qs.len());
    let src = source(&qs);
    let mut bad = Vec::new();
    for (cc, compat) in ORACLES {
        if Command::new(cc).arg("--version").output().is_err() {
            continue;
        }
        for triple in TRIPLES {
            for (c, spelling, lex) in CSTDS {
                for gnu in [false, true] {
                    let std = format!("{}{spelling}", if gnu { "gnu" } else { "c" });
                    let Some(want) = clang(cc, &src, triple, &std, flag) else { return };
                    let mut args: Vec<&str> = flag.into_iter().collect();
                    if gnu {
                        args.push("-std=gnu++");
                    }
                    let f = ClangFeatures::from_args(triple, lex, &args)
                        .expect("supported triple")
                        .with_compat(compat)
                        .with_c(c, gnu);
                    let got = kiln(&src, Box::new(f), triple, c, lex, gnu);
                    let (w, g) = (values(&want), values(&got));
                    for (i, q) in qs.iter().enumerate() {
                        // The ACLE header builtins answer clang 19's 0 at
                        // either level, as in C++ (spec/pp.md, WP-846).
                        let acle = compat == ClangCompat::Clang22
                            && ACLE.iter().any(|n| *q == format!("__has_builtin({n})"));
                        if w.get(&i) != g.get(&i) && !acle {
                            bad.push(format!(
                                "{cc} {triple} {std} {}: {q}: clang {:?}, kiln {:?}",
                                flag.unwrap_or(""),
                                w.get(&i),
                                g.get(&i)
                            ));
                        }
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{} mismatches:\n{}", bad.len(), bad[..bad.len().min(60)].join("\n"));
}

/// The trait default answers C23's standard attributes (C23 6.7.13.1).
#[test]
fn default_c_attribute_values() {
    let f = NoFeatures;
    assert_eq!(f.has_c_attribute(None, "nodiscard"), 202_003);
    assert_eq!(f.has_c_attribute(None, "__deprecated__"), 201_904);
    assert_eq!(f.has_c_attribute(None, "fallthrough"), 201_910);
    assert_eq!(f.has_c_attribute(None, "maybe_unused"), 202_106);
    assert_eq!(f.has_c_attribute(None, "noreturn"), 202_202);
    assert_eq!(f.has_c_attribute(None, "_Noreturn"), 202_202);
    assert_eq!(f.has_c_attribute(None, "likely"), 0);
    assert_eq!(f.has_c_attribute(Some("gnu"), "unused"), 0);
}

/// `with_c` answers C's values for a C-only key and leaves C++ alone.
#[test]
fn with_c_applies_only_in_c() {
    let t = "x86_64-unknown-linux-gnu";
    let cxx = ClangFeatures::new(t, Std::Cxx11, &[]).unwrap().with_compat(ClangCompat::Clang19);
    let c = cxx.clone().with_c(CStd::C11, false);
    assert!(cxx.has_feature("cxx_rtti"));
    assert!(!c.has_feature("cxx_rtti"));
    assert!(!cxx.is_keyword("_Bool"));
    assert!(c.is_keyword("_Bool"));
    assert!(!c.is_keyword("class"));
    assert_eq!(c.has_c_attribute(Some("__gnu__"), "__unused__"), 1);
    assert_eq!(c.has_c_attribute(None, "nodiscard"), 202_003);
    assert_eq!(cxx.has_c_attribute(None, "nodiscard"), 0);
}
