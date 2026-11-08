//! The mangling of reflection template arguments and the emission of
//! `std::meta::info`-templated entities (spec/reflection.md §8.3,
//! WP-501). kiln only: clang 19/22 lack `^^`, so the IR is checked
//! against pinned strings rather than an oracle (the per-kind table and
//! the cross-TU and GCC link tests are `kiln_driver/tests/
//! reflection_mangling.rs`).
//!
//! The default grammar: `<expr-primary> ::= L u6__info <kind> <entity> E`,
//! with `u6__info` a substitution candidate and `<kind> <entity>` GCC
//! 16.1's reflection encoding, so the string is a function of the
//! reflectee alone and the specializations are `linkonce_odr`.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::sync::Arc;

use kiln_abi::mangle::{AnonIds, mangle_decl};
use kiln_abi::{Arch, ReflectionMangling, Target};
use kiln_ast::decl::DeclKind;
use kiln_codegen::{CodegenOptions, emit};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` with `-freflection` and lowers it; returns the IR text
/// and the symbols of the function template specializations named `name`
/// mangled twice over one fresh [`AnonIds`], with its literal count.
fn lower(src: &str, name: &str) -> (String, Vec<String>, Vec<String>, u64) {
    lower_as(src, name, ReflectionMangling::Kiln)
}

/// [`lower`] with reflection mangling form `mode`.
fn lower_as(
    src: &str,
    name: &str,
    mode: ReflectionMangling,
) -> (String, Vec<String>, Vec<String>, u64) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let target = Target::for_arch(Arch::AArch64);
    let mut tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        target.clone(),
        &ParseOptions { reflection: true, ..ParseOptions::default() },
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let opts = CodegenOptions {
        source_filename: "t.cpp".to_owned(),
        reflection_mangling: mode,
        ..CodegenOptions::default()
    };
    let m = emit(&mut tu.ast, &mut tu.oven, &target, &opts).expect("codegen");
    let ir = m.to_string();
    let ast = &tu.ast;
    let specs: Vec<_> = ast
        .decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::Function
                && ast.spec_info(d).is_some()
                && kiln_abi::mangle::decl_name(ast, d) == name
        })
        .collect();
    let mut anon = AnonIds::with_reflection_mangling(mode);
    let first: Vec<String> = specs.iter().map(|&d| mangle_decl(ast, &mut anon, d)).collect();
    let second: Vec<String> = specs.iter().map(|&d| mangle_decl(ast, &mut anon, d)).collect();
    (ir, first, second, anon.reflection_literals())
}

/// The `define`d or global symbols of `ir` starting with `prefix`, with
/// their linkage word.
fn symbols(ir: &str, prefix: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for line in ir.lines() {
        let (link, rest) = if let Some(r) = line.strip_prefix("define ") {
            let w = r.split_whitespace().next().unwrap_or("");
            (w.to_owned(), r)
        } else if let Some(r) = line.strip_prefix('@') {
            let w = r.split(" = ").nth(1).and_then(|s| s.split_whitespace().next()).unwrap_or("");
            (w.to_owned(), line)
        } else {
            continue;
        };
        if let Some(at) = rest.find(&format!("@{prefix}")) {
            let s: String = rest[at + 1..]
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
                .collect();
            out.push((s, link));
        }
    }
    out
}

const SHAPES: &str = r"
using info = decltype(^^int);
using I = int;
using J = int;
struct P { int x; double y; };
template <info R> int f() { static int n = 0; return ++n; }
template <info R> int v = 1;
template <info R> struct S {
  static int count;
  virtual int get() const { return 1; }
};
template <info R> int S<R>::count = 0;
template <info A, info B> int two() { return 2; }
template <class T> struct Box {};
template <info R, class T> int h(Box<T>, T) { return 3; }
int use() {
  S<^^int> s;
  S<^^I>::count++;
  S<^^int>::count++;
  return f<^^int>() + f<^^I>() + f<^^J>() + f<^^int>() + v<^^P::x> + v<^^P::y>
       + s.get() + two<^^I, ^^J>() + two<^^int, ^^int>() + h<^^P>(Box<P>{}, P{});
}
";

/// [temp.type]/2, [basic.link] P2996R13: `^^int`, `^^I` and `^^J` are
/// three specializations, each spelled by its reflectee alone and defined
/// `linkonce_odr` (one per program).
#[test]
fn distinct_reflections_distinct_linkonce_symbols() {
    let (ir, first, _, _) = lower(SHAPES, "f");
    let f = symbols(&ir, "_Z1fILu6__info");
    let defs: BTreeSet<_> =
        f.iter().filter(|(s, _)| s.ends_with("Eiv")).map(|(s, _)| s.as_str()).collect();
    let want: BTreeSet<_> =
        ["_Z1fILu6__infotyiEEiv", "_Z1fILu6__infota1I_iEEiv", "_Z1fILu6__infota1J_iEEiv"].into();
    assert_eq!(defs, want, "{f:?}");
    assert_eq!(first.len(), 3);
    assert!(f.iter().all(|(_, l)| l == "linkonce_odr"), "{f:?}");
    // The static of each specialization is its own object, shared across
    // TUs like the function.
    let statics: BTreeSet<_> = symbols(&ir, "_ZZ1fILu6__info").into_iter().collect();
    let want: BTreeSet<_> = [
        "_ZZ1fILu6__infotyiEEivE1n",
        "_ZZ1fILu6__infota1I_iEEivE1n",
        "_ZZ1fILu6__infota1J_iEEivE1n",
    ]
    .into_iter()
    .map(|s| (s.to_owned(), "linkonce_odr".to_owned()))
    .collect();
    assert_eq!(statics, want);
    // The variable template, the class template's static member, vtable
    // and member function.
    for want in [
        "_Z1vILu6__infodm1P1xEE",
        "_Z1vILu6__infodm1P1yEE",
        "_ZN1SILu6__infotyiEE5countE",
        "_ZN1SILu6__infota1I_iEE5countE",
        "_ZTV1SILu6__infotyiEE",
        "_ZNK1SILu6__infotyiEE3getEv",
    ] {
        let s = symbols(&ir, want);
        assert!(!s.is_empty() && s.iter().all(|(_, l)| l == "linkonce_odr"), "{want}: {s:?}");
    }
}

/// A second reflection literal in one symbol reuses the info type
/// through the substitution table (`S_` is the template name `two`, `S0_`
/// the info type); the reflectee's own components are candidates too.
#[test]
fn second_literal_uses_substitution() {
    let (ir, ..) = lower(SHAPES, "two");
    let two: BTreeSet<_> = symbols(&ir, "_Z3twoILu6__info").into_iter().map(|(s, _)| s).collect();
    let want: BTreeSet<_> =
        ["_Z3twoILu6__infotyiELS0_tyiEEiv", "_Z3twoILu6__infota1I_iELS0_ta1J_iEEiv"]
            .into_iter()
            .map(str::to_owned)
            .collect();
    assert_eq!(two, want);
    // In `h<^^P>(Box<P>, P)`: S_ = h, S0_ = u6__info, S1_ = P (inside the
    // literal), so T0_ = S1_; S2_ = Box, S3_ = T0_, S4_ = Box<T0_>; the
    // second parameter is S3_.
    let h: Vec<_> = symbols(&ir, "_Z1hILu6__info").into_iter().map(|(s, _)| s).collect();
    assert!(
        !h.is_empty() && h.iter().all(|s| s == "_Z1hILu6__infoty1PES1_Ei3BoxIT0_ES3_"),
        "{h:?}"
    );
}

/// `-fkiln-reflection-mangling=gcc` spells the info type `Dm` (a builtin,
/// not a substitution candidate) and the literal `L Dm <kind> <entity>
/// E`; `kiln-hashed` writes the kiln form's body as a 128-bit FNV-1a
/// hash (`h <32 hex>`).
#[test]
fn gcc_and_hashed_forms() {
    let (ir, ..) = lower_as(SHAPES, "two", ReflectionMangling::Gcc);
    let two: BTreeSet<_> = symbols(&ir, "_Z3twoILDm").into_iter().map(|(s, _)| s).collect();
    let want: BTreeSet<_> = ["_Z3twoILDmtyiELDmtyiEEiv", "_Z3twoILDmta1I_iELDmta1J_iEEiv"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    assert_eq!(two, want);
    assert!(symbols(&ir, "_Z1hILDmty1PES0_Ei3BoxIT0_ES2_").len() == 1, "{ir}");
    let (ir, first, second, _) = lower_as(SHAPES, "f", ReflectionMangling::KilnHashed);
    assert_eq!(first, second);
    let f: BTreeSet<_> = symbols(&ir, "_Z1fILu6__infoh").into_iter().map(|(s, _)| s).collect();
    assert_eq!(f.len(), 3, "{f:?}");
    for s in &f {
        let hex = s.strip_prefix("_Z1fILu6__infoh").and_then(|t| t.strip_suffix("EEiv"));
        assert!(
            hex.is_some_and(|h| h.len() == 32 && h.bytes().all(|b| b.is_ascii_hexdigit())),
            "{s}"
        );
    }
}

/// Mangling is a function of the TU: emitting it twice gives the same IR.
#[test]
fn deterministic() {
    let (a, ..) = lower(SHAPES, "f");
    let (b, ..) = lower(SHAPES, "f");
    assert_eq!(a, b);
}

/// 2,000 specializations differing only in the reflection: 2,000 distinct
/// `linkonce_odr` symbols, equal on a second mangling, and the mangler's
/// literal counter (`AnonIds::reflection_literals`) counts one literal
/// per symbol.
#[test]
fn two_thousand_specializations() {
    const N: usize = 2000;
    let mut src =
        String::from("using info = decltype(^^int);\ntemplate <info R> int spec() { return 1; }\n");
    for i in 0..N {
        let _ = writeln!(src, "struct C{i} {{}};");
    }
    src.push_str("int (*table[])() = {\n");
    for i in 0..N {
        let _ = writeln!(src, "  &spec<^^C{i}>,");
    }
    src.push_str("};\n");
    let (ir, first, second, literals) = lower(&src, "spec");
    assert_eq!(first.len(), N);
    assert_eq!(first, second, "a second mangling is identical");
    let distinct: BTreeSet<_> = first.iter().collect();
    assert_eq!(distinct.len(), N);
    assert_eq!(literals, 2 * N as u64);
    let defs = symbols(&ir, "_Z4specILu6__info");
    assert_eq!(defs.iter().filter(|(_, l)| l == "linkonce_odr").count(), N);
    assert!(first.iter().all(|s| s.starts_with("_Z4specILu6__infoty")), "{:?}", &first[..2]);
}

/// [expr.typeid]/1 (WP-879): the C++ runtime defines no `type_info` for
/// `std::meta::info`, so `typeid(info)` defines it where used, a weak
/// `linkonce_odr` `__fundamental_type_info` in a comdat, as g++-16 does
/// for `_ZTIDm`. In the default (kiln) form it is `_ZTIu6__info`; nothing
/// else in the module names the info type.
#[test]
fn info_type_info_is_a_local_weak_definition() {
    let src = r"namespace std { class type_info { public: const char* name() const; }; }
using info = decltype(^^int);
const std::type_info& ti() { return typeid(info); }
";
    for (mode, m) in [(ReflectionMangling::Kiln, "u6__info"), (ReflectionMangling::Gcc, "Dm")] {
        let (ir, ..) = lower_as(src, "ti", mode);
        let lines: Vec<&str> = ir.lines().filter(|l| l.contains(m)).collect();
        let n = m.len() + 1;
        let want = [
            format!("$_ZTI{m} = comdat any"),
            format!("$_ZTS{m} = comdat any"),
            format!(
                "@_ZTI{m} = linkonce_odr dso_local constant {{ ptr, ptr }} {{ ptr getelementptr \
                 inbounds (ptr, ptr @_ZTVN10__cxxabiv123__fundamental_type_infoE, i64 2), ptr \
                 @_ZTS{m} }}, comdat, align 8"
            ),
            format!(
                "@_ZTS{m} = linkonce_odr dso_local constant [{n} x i8] c\"{m}\\00\", comdat, align 1"
            ),
            format!("  ret ptr @_ZTI{m}"),
        ];
        assert_eq!(lines, want.iter().map(String::as_str).collect::<Vec<_>>(), "{mode:?}: {ir}");
    }
}
