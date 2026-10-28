//! WP-015: [`ClangFeatures`] against clang 19 — the feature-test hooks of
//! [cpp.cond]/5 (`__has_cpp_attribute`, Table 23 [tab:cpp.cond.ha]) and
//! clang's extensions (`__has_feature`, `__has_extension`,
//! `__has_builtin`, `__has_constexpr_builtin`, `__has_attribute`,
//! `__has_declspec_attribute`, `__is_identifier`, `__is_target_*`,
//! `__FLT_EVAL_METHOD__`).
//!
//! The differential tests query every key of the generated table (plus
//! absent and `__x__`-spelled names) through `kiln_pp` with `ClangFeatures`
//! installed and through `clang++-19 -E -undef`, and require identical
//! answers. They are skipped when clang is not installed.

mod ifeval_common;

use std::fmt::Write as _;

use std::collections::BTreeMap;

use ifeval_common::{clang_available, run_clang, run_kiln_with};
use kiln_lex::Std;
use kiln_pp::{ClangFeatures, FeatureFlag, FeatureQueries, MemoryResolver, TargetQuery};

const TABLE: &str = include_str!("../src/features_table.rs");
const TRIPLES: [&str; 2] = ["aarch64-unknown-linux-gnu", "x86_64-unknown-linux-gnu"];
const STDS: [Std; 8] = [
    Std::Cxx98,
    Std::Cxx03,
    Std::Cxx11,
    Std::Cxx14,
    Std::Cxx17,
    Std::Cxx20,
    Std::Cxx23,
    Std::Cxx26,
];

/// The query spelling for a table kind.
fn query(kind: &str, key: &str) -> String {
    let hook = match kind {
        "FEATURE" | "Feature" => "__has_feature",
        "EXTENSION" | "Extension" => "__has_extension",
        "BUILTIN" | "Builtin" => "__has_builtin",
        "CONSTEXPR_BUILTIN" | "ConstexprBuiltin" => "__has_constexpr_builtin",
        "ATTRIBUTE" | "Attribute" => "__has_attribute",
        "DECLSPEC_ATTRIBUTE" | "DeclspecAttribute" => "__has_declspec_attribute",
        "CPP_ATTRIBUTE" | "CppAttribute" => "__has_cpp_attribute",
        "KEYWORD" | "Keyword" => "__is_identifier",
        _ => panic!("unknown kind {kind}"),
    };
    format!("{hook}({key})")
}

/// The first string literal on `line`.
fn quoted(line: &str) -> &str {
    let a = line.find('"').unwrap() + 1;
    let b = a + line[a..].find('"').unwrap();
    &line[a..b]
}

/// Every base-table query `(kind, key)` of the generated table.
fn table_queries() -> Vec<String> {
    let mut out = Vec::new();
    let mut kind = "";
    for line in TABLE.lines() {
        if let Some(rest) = line.strip_prefix("pub(super) static ") {
            kind = rest.split(':').next().unwrap();
        } else if line.trim_start().starts_with("Entry(") {
            out.push(query(kind, quoted(line)));
        }
    }
    out.dedup();
    out
}

/// Every overlay query, with its flag's name.
fn overlay_queries() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in TABLE.lines() {
        if let Some(rest) = line.trim_start().strip_prefix("Overlay(FeatureFlag::") {
            let mut it = rest.split(", ");
            let flag = it.next().unwrap().to_owned();
            let kind = it.next().unwrap().strip_prefix("Kind::").unwrap();
            out.push((flag, query(kind, quoted(line))));
        }
    }
    out
}

/// Names no table holds, and normalized spellings of some that it does.
fn extra_queries() -> Vec<String> {
    let mut q: Vec<String> = [
        "__has_feature(no_such_feature)",
        "__has_feature(__cxx_exceptions__)",
        "__has_feature(__cxx_rtti__)",
        "__has_feature(__attribute_deprecated_with_message__)",
        "__has_extension(__cxx_variadic_templates__)",
        "__has_extension(no_such_extension)",
        "__has_builtin(__builtin_no_such_builtin)",
        "__has_builtin(__builtin_expect)",
        "__has_builtin(__builtin_operator_new)",
        "__has_builtin(__builtin_operator_delete)",
        "__has_builtin(__make_integer_seq)",
        "__has_builtin(__type_pack_element)",
        "__has_builtin(__is_target_arch)",
        "__has_builtin(__is_target_os)",
        "__has_builtin(__has_builtin)",
        "__has_builtin(__is_same)",
        "__has_builtin(__builtin_expect__)",
        "__has_builtin(printf)",
        "__has_constexpr_builtin(__builtin_strlen)",
        "__has_constexpr_builtin(__builtin_no_such)",
        "__has_attribute(__always_inline__)",
        "__has_attribute(__noreturn__)",
        "__has_attribute(no_such_attribute)",
        "__has_attribute(__no_such_attribute__)",
        "__has_declspec_attribute(dllexport)",
        "__has_declspec_attribute(__dllexport__)",
        "__has_cpp_attribute(__nodiscard__)",
        "__has_cpp_attribute(__fallthrough__)",
        "__has_cpp_attribute(gnu::__always_inline__)",
        "__has_cpp_attribute(__gnu__::always_inline)",
        "__has_cpp_attribute(__gnu__::__always_inline__)",
        "__has_cpp_attribute(_Clang::fallthrough)",
        "__has_cpp_attribute(clang::__fallthrough__)",
        "__has_cpp_attribute(_Clang::__fallthrough__)",
        "__has_cpp_attribute(msvc::__no_unique_address__)",
        "__has_cpp_attribute(gsl::Owner)",
        "__has_cpp_attribute(no_such::attribute)",
        "__has_cpp_attribute(clang::no_such_attribute)",
        "__is_identifier(foo)",
        "__is_identifier(int)",
        "__is_identifier(and)",
        "__is_identifier(char8_t)",
        "__is_identifier(__is_same)",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    q.push("__FLT_EVAL_METHOD__".to_owned());
    q
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

/// Mismatches between kiln (with `ClangFeatures::from_args(triple, std,
/// args)`, no predefines) and `clang -undef args` on `queries`.
fn compare(triple: &str, std: Std, args: &[&str], queries: &[String]) -> Vec<String> {
    let mut src = String::new();
    for (i, q) in queries.iter().enumerate() {
        writeln!(src, "L{i} {q}").unwrap();
    }
    let mut cargs = vec!["-undef"];
    cargs.extend_from_slice(args);
    let clang = run_clang(&src, triple, std, &cargs).expect("clang runs");
    let features = ClangFeatures::from_args(triple, std, args).expect("supported triple");
    let kiln = run_kiln_with(&src, triple, std, false, &[], MemoryResolver::new(), |pp| {
        pp.set_feature_queries(Box::new(features));
    });
    let (c, k) = (values(&clang.text), values(&kiln.text));
    let mut bad = Vec::new();
    for (i, q) in queries.iter().enumerate() {
        if c.get(&i) != k.get(&i) {
            bad.push(format!(
                "{triple} {std:?} {args:?}: {q}: clang {:?}, kiln {:?}",
                c.get(&i),
                k.get(&i)
            ));
        }
    }
    if !clang.diags.is_empty() {
        bad.push(format!("{triple} {std:?} {args:?}: clang diagnostics {:?}", clang.diags));
    }
    bad
}

fn report(bad: &[String]) {
    assert!(
        bad.is_empty(),
        "{} mismatches:\n{}",
        bad.len(),
        bad.iter().take(60).cloned().collect::<Vec<_>>().join("\n")
    );
}

/// Every table key, in every target and standard, without flags.
#[test]
fn table_matches_clang_in_every_configuration() {
    if !clang_available() {
        return;
    }
    let mut qs = table_queries();
    assert!(qs.len() > 3000, "table has {} keys", qs.len());
    qs.extend(extra_queries());
    qs.extend(overlay_queries().into_iter().map(|(_, q)| q));
    qs.sort();
    qs.dedup();
    let mut bad = Vec::new();
    for triple in TRIPLES {
        for std in STDS {
            bad.extend(compare(triple, std, &[], &qs));
        }
    }
    report(&bad);
}

/// clang's options for `flag` in `std` (`-std=gnu++NN` for `GnuMode`).
fn flag_args(flag: FeatureFlag, std: Std) -> Vec<String> {
    if flag == FeatureFlag::GnuMode {
        let s = ifeval_common::std_flag(std).replace("c++", "gnu++");
        return vec![s];
    }
    flag.options().iter().map(|s| (*s).to_owned()).collect()
}

/// Each flag alone, in the oldest and newest standard, over every key any
/// flag changes.
#[test]
fn each_flag_matches_clang() {
    if !clang_available() {
        return;
    }
    let mut qs: Vec<String> = overlay_queries().into_iter().map(|(_, q)| q).collect();
    qs.extend(extra_queries());
    qs.sort();
    qs.dedup();
    let mut bad = Vec::new();
    for &flag in FeatureFlag::ALL {
        for triple in TRIPLES {
            // Flags clang rejects for the target are not probed.
            let opts = flag.options().join(" ");
            if triple.starts_with("x86_64") && opts.contains("ptrauth") {
                continue;
            }
            if triple.starts_with("aarch64") && opts.contains("numerical") {
                continue;
            }
            for std in [Std::Cxx98, Std::Cxx26] {
                let args = flag_args(flag, std);
                let args: Vec<&str> = args.iter().map(String::as_str).collect();
                bad.extend(compare(triple, std, &args, &qs));
            }
        }
    }
    report(&bad);
}

/// Several flags at once: overlays compose.
#[test]
fn flag_combinations_match_clang() {
    if !clang_available() {
        return;
    }
    let mut qs: Vec<String> = overlay_queries().into_iter().map(|(_, q)| q).collect();
    qs.extend(extra_queries());
    qs.sort();
    qs.dedup();
    let combos: [&[&str]; 8] = [
        &["-fno-exceptions", "-fno-rtti"],
        &["-fms-extensions", "-fblocks", "-fno-operator-names"],
        &["-fsanitize=address,undefined"],
        &["-fms-compatibility", "-fdeclspec", "-fno-gnu-keywords"],
        &["-fmodules", "-pedantic-errors"],
        &["-fno-exceptions", "-fexceptions", "-fno-rtti", "-frtti"],
        &["-fsanitize=thread", "-fno-sanitize=thread", "-fsanitize=memory"],
        &["-fno-char8_t", "-fno-coroutines", "-fno-builtin", "-fopenmp"],
    ];
    let mut bad = Vec::new();
    for triple in TRIPLES {
        for std in [Std::Cxx11, Std::Cxx20] {
            for args in combos {
                bad.extend(compare(triple, std, args, &qs));
            }
            bad.extend(compare(
                triple,
                std,
                &[
                    if std == Std::Cxx11 { "-std=gnu++11" } else { "-std=gnu++20" },
                    "-fms-extensions",
                ],
                &qs,
            ));
        }
    }
    report(&bad);
}

/// Operands of `__has_*` queries in the installed libc++ 19, glibc and
/// clang builtin headers.
fn header_queries() -> Vec<String> {
    let mut out = Vec::new();
    let dirs = [
        "/usr/lib/llvm-19/include/c++/v1",
        "/usr/include",
        "/usr/lib/llvm-19/lib/clang/19/include",
    ];
    let mut stack: Vec<std::path::PathBuf> = dirs.iter().map(Into::into).collect();
    while let Some(p) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&p) else {
            if let Ok(bytes) = std::fs::read(&p) {
                scan(&String::from_utf8_lossy(&bytes), &mut out);
            }
            continue;
        };
        for e in rd.flatten() {
            stack.push(e.path());
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Appends each `__has_<hook>(operand)` in `text` to `out`.
fn scan(text: &str, out: &mut Vec<String>) {
    const HOOKS: [&str; 7] = [
        "__has_builtin",
        "__has_feature",
        "__has_extension",
        "__has_attribute",
        "__has_cpp_attribute",
        "__has_declspec_attribute",
        "__has_constexpr_builtin",
    ];
    let mut rest = text;
    while let Some(i) = rest.find("__has_") {
        rest = &rest[i..];
        let hook = HOOKS.iter().find(|h| {
            rest.starts_with(**h)
                && rest[h.len()..].trim_start().starts_with('(')
                && !rest[h.len()..].starts_with(|c: char| c.is_alphanumeric() || c == '_')
        });
        if let Some(h) = hook {
            let after = rest[h.len()..].trim_start()[1..].trim_start();
            let end = after.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == ':'));
            if let Some(end) = end {
                let operand = &after[..end];
                let ok = !operand.is_empty()
                    && !operand.starts_with(|c: char| c.is_ascii_digit())
                    && (operand.matches("::").count() == 1 && *h == "__has_cpp_attribute"
                        || !operand.contains(':'))
                    && after[end..].trim_start().starts_with(')');
                if ok {
                    out.push(format!("{h}({operand})"));
                }
            }
        }
        rest = &rest[6..];
    }
}

/// Every `__has_*` operand the libc++ 19, glibc and clang headers ask
/// about answers as in clang (WP-015 obligation: at least every
/// `__has_builtin` name used by libc++ 19 and glibc). Operands that are
/// macros in normal compilation are queried as written (`-undef`).
#[test]
fn header_queries_match_clang() {
    if !clang_available() {
        return;
    }
    let qs = header_queries();
    if qs.is_empty() {
        return;
    }
    let builtins = qs.iter().filter(|q| q.starts_with("__has_builtin(")).count();
    assert!(builtins > 50, "only {builtins} __has_builtin operands found");
    let mut bad = Vec::new();
    for triple in TRIPLES {
        for std in [Std::Cxx03, Std::Cxx11, Std::Cxx17, Std::Cxx20, Std::Cxx23, Std::Cxx26] {
            bad.extend(compare(triple, std, &[], &qs));
        }
    }
    report(&bad);
}

/// `__is_target_*` names, against clang ([cpp.cond]: clang extension).
#[test]
fn is_target_matches_clang() {
    if !clang_available() {
        return;
    }
    let names = [
        "aarch64",
        "AArch64",
        "arm64",
        "ARM64",
        "arm64e",
        "arm64_32",
        "aarch64_be",
        "aarch64_32",
        "arm",
        "thumb",
        "x86_64",
        "X86_64",
        "x86_64h",
        "amd64",
        "AMD64",
        "i386",
        "x86",
        "unknown",
        "Unknown",
        "pc",
        "apple",
        "linux",
        "Linux",
        "linuxfoo",
        "linux_gnu",
        "darwin",
        "macos",
        "ios",
        "windows",
        "win32",
        "freebsd",
        "gnu",
        "GNU",
        "gnufoo",
        "gnueabi",
        "gnueabihf",
        "gnux32",
        "gnu_ilp32",
        "gnuabi64",
        "gnuabin32",
        "gnuf32",
        "gnuf64",
        "gnusf",
        "gnut64",
        "eabi",
        "musl",
        "android",
        "msvc",
        "macabi",
        "simulator",
        "code16",
        "none",
        "elf",
    ];
    let hooks = [
        "__is_target_arch",
        "__is_target_vendor",
        "__is_target_os",
        "__is_target_environment",
        "__is_target_variant_os",
        "__is_target_variant_environment",
    ];
    let qs: Vec<String> =
        hooks.iter().flat_map(|h| names.iter().map(move |n| format!("{h}({n})"))).collect();
    let mut bad = Vec::new();
    for triple in TRIPLES {
        bad.extend(compare(triple, Std::Cxx26, &[], &qs));
    }
    report(&bad);
}

// ---- unit tests (no clang needed) -------------------------------------------

fn cf(triple: &str, std: Std, args: &[&str]) -> ClangFeatures {
    ClangFeatures::from_args(triple, std, args).unwrap()
}

/// [cpp.cond]/5, Table 23: the standard attributes' values.
#[test]
fn standard_attribute_values() {
    let f = cf("x86_64-unknown-linux-gnu", Std::Cxx26, &[]);
    for (name, v) in [
        ("assume", 202_207),
        ("carries_dependency", 200_809),
        ("deprecated", 201_309),
        ("fallthrough", 201_603),
        ("likely", 201_803),
        ("maybe_unused", 201_603),
        ("no_unique_address", 201_803),
        ("nodiscard", 201_907),
        ("noreturn", 200_809),
        ("unlikely", 201_803),
        ("__nodiscard__", 201_907),
    ] {
        assert_eq!(f.has_cpp_attribute(None, name), v, "{name}");
    }
    assert_eq!(f.has_cpp_attribute(Some("__gnu__"), "__always_inline__"), 1);
    // Scoped spellings name the vendor attribute, which has no date.
    assert_eq!(f.has_cpp_attribute(Some("_Clang"), "fallthrough"), 1);
    assert_eq!(f.has_cpp_attribute(Some("nosuch"), "fallthrough"), 0);
}

/// clang extensions: representative values and flag dependence.
#[test]
fn clang_extension_values() {
    let x = "x86_64-unknown-linux-gnu";
    let f = cf(x, Std::Cxx26, &[]);
    assert!(f.has_feature("cxx_exceptions") && f.has_feature("__cxx_exceptions__"));
    assert!(f.has_feature("cxx_rtti"));
    assert!(!f.has_feature("address_sanitizer"));
    assert_eq!(f.has_builtin("__builtin_operator_new"), 201_802);
    assert_eq!(f.has_builtin("__builtin_expect"), 1);
    assert_eq!(f.has_builtin("__builtin_expect__"), 0, "no normalization for builtins");
    assert!(f.has_constexpr_builtin("__builtin_strlen"));
    assert_eq!(f.has_attribute("__always_inline__"), 1);
    assert!(!f.has_declspec_attribute("noinline"), "needs -fms-extensions or -fdeclspec");
    assert!(f.is_keyword("int") && f.is_keyword("__is_same") && !f.is_keyword("foo"));
    assert_eq!(f.flt_eval_method(), 0);

    let g = cf(x, Std::Cxx26, &["-fno-exceptions", "-fsanitize=address,undefined", "-fno-rtti"]);
    assert!(!g.has_feature("cxx_exceptions"));
    assert!(!g.has_feature("cxx_rtti"));
    assert!(g.has_feature("address_sanitizer"));
    let h = cf(x, Std::Cxx26, &["-fno-exceptions", "-fexceptions", "-fms-extensions"]);
    assert!(h.has_feature("cxx_exceptions"));
    assert!(h.has_declspec_attribute("noinline") && h.has_declspec_attribute("align"));
    // dllexport is a Windows-only target attribute.
    assert!(!h.has_declspec_attribute("dllexport"));
    assert!(!cf(x, Std::Cxx26, &["-fno-operator-names"]).is_keyword("and"));
    assert!(cf(x, Std::Cxx26, &[]).is_keyword("and"));
    // C++11 keywords are identifiers in C++98 (clang warns elsewhere).
    assert!(!cf(x, Std::Cxx98, &[]).is_keyword("nullptr"));
    assert!(cf(x, Std::Cxx11, &[]).is_keyword("nullptr"));
    // Extensions become features only under -pedantic-errors.
    assert!(cf(x, Std::Cxx98, &[]).has_extension("cxx_variadic_templates"));
    assert!(!cf(x, Std::Cxx98, &["-pedantic-errors"]).has_extension("cxx_variadic_templates"));
}

/// Triples: vendor-less spellings, unsupported targets, per-target flags.
#[test]
fn triples_and_target_flags() {
    assert!(ClangFeatures::new("x86_64-linux-gnu", Std::Cxx26, &[]).is_some());
    assert!(ClangFeatures::new("aarch64-unknown-linux-gnu", Std::Cxx26, &[]).is_some());
    assert!(ClangFeatures::new("x86_64-pc-windows-msvc", Std::Cxx26, &[]).is_none());
    assert!(ClangFeatures::new("riscv64-unknown-linux-gnu", Std::Cxx26, &[]).is_none());
    let a = cf("aarch64-unknown-linux-gnu", Std::Cxx26, &["-fptrauth-intrinsics"]);
    assert_eq!(a.flags(), &[FeatureFlag::PtrauthIntrinsics]);
    assert!(a.has_feature("ptrauth_intrinsics"));
    let x = cf("x86_64-unknown-linux-gnu", Std::Cxx26, &["-fptrauth-intrinsics"]);
    assert!(x.flags().is_empty(), "clang rejects -fptrauth-* for x86_64");
    assert!(x.is_target(TargetQuery::Arch, "AMD64"));
    assert!(x.is_target(TargetQuery::Vendor, "UNKNOWN"));
    assert!(x.is_target(TargetQuery::Os, "linux"));
    assert!(x.is_target(TargetQuery::Environment, "gnu"));
    assert!(!x.is_target(TargetQuery::Environment, "gnux32"));
    assert!(!x.is_target(TargetQuery::VariantOs, "linux"));
    assert!(a.is_target(TargetQuery::Arch, "arm64"));
    assert!(!a.is_target(TargetQuery::Arch, "x86_64"));
}
