//! The syntactic not-TU-local test of the link-time metadata's type rows
//! (spec/codegen.md "Link-time metadata (`.mason.meta`)", kind 3; mortar
//! `spec/odr.md` O-17; [basic.link]/13-15), checked two ways:
//!
//! - named cases: the classes O-17 lists and the clauses' edge cases;
//! - the cross-check O-17 demands: over every class and enumeration
//!   definition of the corpus, the test is false whenever the type's
//!   `<type>` encoding has a per-TU component (`_GLOBAL__N`, a local-entity
//!   `Z...E`, an unnamed `Ut`/`Ul`, clang's `$_N`). The reverse (a failing
//!   type without one) is allowed (mortar's X18); those are counted and
//!   printed with `--nocapture`.
//!
//! The same direction over the rows of library-heavy units is in
//! `crates/kiln_driver/tests/mason_meta.rs` (`type_rows_have_no_per_tu_component`).

use std::collections::BTreeMap;
use std::sync::Arc;

use kiln_abi::mangle;
use kiln_abi::odr::{NotTuLocal, is_templated};
use kiln_abi::{Arch, Target};
use kiln_ast::Ast;
use kiln_ast::decl::DeclKind;
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, ParsedTu, parse_tu};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

fn parse(src: &str) -> ParsedTu {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let tu = parse_tu(
        &sm,
        &interner,
        file,
        &engine,
        Target::for_arch(Arch::X86_64),
        &ParseOptions::default(),
        ConstexprLimits::default(),
    );
    let errors: Vec<String> =
        sink.take().into_iter().filter(|d| d.level >= Level::Error).map(|d| d.message).collect();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    tu
}

/// The per-TU component of a `<type>` encoding, if any: a source name
/// `_GLOBAL__N...` (unnamed namespace) or `$...` (clang's unnumbered
/// closure), a local name `Z`, an unnamed type `Ut`/`Ul`. A small reader
/// of the encodings the corpus produces: source names are skipped by
/// their length, literals `L <builtin> <number> E` as a whole.
fn per_tu_component(enc: &str) -> Option<&'static str> {
    let b = enc.as_bytes();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && b[i].is_ascii_digit() {
                i += 1;
            }
            let n: usize = enc[start..i].parse().unwrap();
            let name = &enc[i..(i + n).min(enc.len())];
            if name.starts_with("_GLOBAL__N") {
                return Some("_GLOBAL__N");
            }
            if name.starts_with('$') {
                return Some("$_N");
            }
            i += n;
            continue;
        }
        match c {
            b'Z' => return Some("Z...E"),
            b'U' if matches!(b.get(i + 1), Some(b't' | b'l')) => return Some("Ut/Ul"),
            // `L_Z <encoding> E`: an external entity's name, not a local one.
            b'L' if b.get(i + 1) == Some(&b'_') && b.get(i + 2) == Some(&b'Z') => i += 2,
            b'L' if b.get(i + 1).is_some_and(u8::is_ascii_lowercase) => {
                // A literal: skip to its `E`.
                while i < b.len() && b[i] != b'E' {
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Every non-templated class and enumeration definition of `ast`: its
/// `<type>` encoding and whether it passes, by encoding (the last wins:
/// a name is one type).
fn verdicts(ast: &Ast) -> BTreeMap<String, (bool, DeclKind, bool)> {
    let anon = &mut mangle::AnonIds::new();
    let mut t = NotTuLocal::new(ast);
    let mut out = BTreeMap::new();
    for d in ast.decls.ids() {
        let k = ast.decls.get(d).kind();
        if !matches!(
            k,
            DeclKind::CXXRecord | DeclKind::ClassTemplateSpecialization | DeclKind::Enum
        ) || ast.definition(d) != Some(d)
            || is_templated(ast, d)
        {
            continue;
        }
        let enc = mangle::mangle_class_special(ast, anon, "", d);
        let unnamed = ast.decls.get(d).name().is_empty();
        out.insert(enc, (t.tag(d), k, unnamed));
    }
    out
}

const CORPUS: &str = r#"
namespace std {
  template<class T> struct optional { T t; bool engaged; };
  template<class A, class B> struct pair { A first; B second; };
  template<class T> struct vector { T* b; T* e; };
  inline namespace __v1 { struct inl { int x; }; }
}
namespace ns {
  struct Gadget { int g; };
  template<class T> struct Widget { T t; };
  enum class Color : unsigned char { red, green };
  enum Plain { a, b };
  template<Color C> struct ByColor { int x; };
  template<int... N> struct Ints { int x; };
  template<class... T> struct Types { int x; };
  template<decltype(nullptr) P> struct Null { int x; };
  template<long long N> struct Big { int x; };
  template<double D> struct Flt { int x; };
  template<const int* P> struct Ptr { int x; };
  template<template<class> class TT> struct TempTemp { int x; };
  template<class T> struct Outer { struct Inner { T t; }; };
  struct WithMember { struct Nested { int n; }; Nested n; };
  extern int global;
  struct Agg { int a; int b; };
}
extern "C++" { struct InLinkageSpec { int x; }; }
struct Global { int g; };
typedef struct { int t; } TypedefNamed;
struct HasAnon { union { int i; float f; }; struct { int x; } named_unnamed; };
enum { unnamed_enumerator };
namespace {
  struct Impl { int i; };
  enum E { e0 };
  namespace inner { struct Deep { int d; }; }
}
namespace ns { namespace { struct Hidden { int h; }; } }
struct [[clang::internal_linkage]] Internal { int x; };

static int local_t() { struct Local { int l; }; Local l{}; return l.l; }
inline int local_inline() { struct LocalInline { int l; }; LocalInline l{}; return l.l; }
struct Method { void m() { struct InMethod { int x; }; InMethod v{}; (void)v; } };
static auto lam = [] { return 1; };
inline auto lam_inline = [] { struct InLambda { int x; }; return InLambda{}; };

// Uses, so every specialization has a definition.
std::optional<int> o1;
std::optional<Impl> o2;
std::optional<ns::Gadget*> o3;
std::vector<std::pair<int, Impl*>> v1;
std::vector<std::pair<int, ns::Gadget>> v2;
ns::Widget<ns::Gadget*> w1;
ns::Widget<const ns::Gadget&(*)(int, ns::Gadget[4])> w2;
ns::Widget<int ns::Gadget::*> w3;
ns::Widget<int (ns::Gadget::*)(Impl)> w4;
ns::Widget<Impl[2]> w5;
ns::Widget<inner::Deep> w6;
ns::Widget<ns::Hidden> w7;
ns::Widget<TypedefNamed> w8;
ns::Widget<E> w9;
ns::Widget<decltype(lam)> w10;
ns::Widget<ns::Widget<Impl>> w11;
ns::Widget<std::optional<ns::Color>> w12;
ns::Widget<std::inl> w13;
ns::Widget<InLinkageSpec> w14;
ns::ByColor<ns::Color::green> c1;
ns::Ints<1, 2, 3> i1;
ns::Types<int, ns::Gadget, Global> t1;
ns::Types<int, Impl> t2;
ns::Null<nullptr> n1;
ns::Big<-5> b1;
ns::Flt<1.5> f1;
ns::Ptr<&ns::global> p1;
ns::TempTemp<std::optional> tt1;
ns::Outer<int>::Inner oi1;
ns::Outer<Impl>::Inner oi2;
ns::Widget<ns::Plain> pl;
ns::Widget<__int128> big;
ns::Widget<int __attribute__((vector_size(16)))> vec;
int use() { return local_t() + local_inline() + lam() + lam_inline().x; }
"#;

#[test]
fn named_cases() {
    let tu = parse(CORPUS);
    let v = verdicts(&tu.ast);
    let pass = |enc: &str| v.get(enc).unwrap_or_else(|| panic!("no type {enc} in {v:#?}")).0;
    // O-17's examples.
    assert!(pass("St8optionalIiE"));
    assert!(pass("N2ns6WidgetIPNS_6GadgetEEE"));
    assert!(!pass("St8optionalIN12_GLOBAL__N_14ImplEE"));
    assert!(!pass("St6vectorISt4pairIiPN12_GLOBAL__N_14ImplEEE"));
    assert!(pass("St6vectorISt4pairIiN2ns6GadgetEEE"));
    // Clause 1: unnamed types and closures.
    for (enc, (ok, _, unnamed)) in &v {
        if *unnamed {
            assert!(!ok, "unnamed type {enc} passes");
        }
    }
    assert!(!pass("12TypedefNamed"), "a typedef name for linkage is not a name of its own");
    assert!(!pass("L8Internal"), "internal_linkage");
    // Clause 2.
    assert!(pass("6Global"));
    assert!(pass("13InLinkageSpec"));
    assert!(pass("NSt4__v13inlE"), "an inline namespace is a named namespace");
    assert!(pass("N2ns10WithMember6NestedE"));
    assert!(!pass("N12_GLOBAL__N_14ImplE"));
    assert!(!pass("N12_GLOBAL__N_15inner4DeepE"));
    assert!(!pass("N2ns12_GLOBAL__N_16HiddenE"), "an unnamed namespace at any depth");
    assert!(!pass("ZL7local_tvE5Local"));
    assert!(!pass("Z12local_inlinevE11LocalInline"), "local classes fail even in inline functions");
    assert!(!pass("ZN6Method1mEvE8InMethod"));
    assert!(!pass("ZNK10lam_inlineMUlvE_clEvE8InLambda"), "a class local to a lambda");
    assert!(!pass("N10lam_inlineMUlvE_E"), "a numbered closure type");
    assert!(!pass("3$_0"), "an unnumbered closure type");
    assert!(!pass("N7HasAnonUt_E") && !pass("N7HasAnonUt0_E"), "anonymous members");
    assert!(pass("7HasAnon"));
    // Clause 3: type arguments.
    assert!(pass("N2ns6WidgetIPFRKNS_6GadgetEiPS1_EEE"), "function types, cv, references");
    assert!(pass("N2ns6WidgetIMNS_6GadgetEiEE"), "member pointers");
    assert!(!pass("N2ns6WidgetIMNS_6GadgetEFiN12_GLOBAL__N_14ImplEEEE"));
    assert!(!pass("N2ns6WidgetIA2_N12_GLOBAL__N_14ImplEEE"));
    assert!(!pass("N2ns6WidgetI12TypedefNamedEE"), "X18: a miss, never a finding");
    assert!(!pass("N2ns6WidgetIN12_GLOBAL__N_11EEEE"));
    assert!(!pass("N2ns6WidgetIN12_GLOBAL__N_15inner4DeepEEE"));
    assert!(!pass("N2ns6WidgetINS_12_GLOBAL__N_16HiddenEEE"));
    // `$_1`: the unnamed enumeration above, without linkage, is `$_0`
    // (clang's numbering; WP-1651).
    assert!(!pass("N2ns6WidgetI3$_1EE"), "a closure type argument");
    assert!(pass("N2ns6WidgetINSt4__v13inlEEE"));
    assert!(pass("N2ns6WidgetI13InLinkageSpecEE"));
    assert!(!pass("N2ns6WidgetINS0_IN12_GLOBAL__N_14ImplEEEEE"));
    assert!(pass("N2ns6WidgetISt8optionalINS_5ColorEEEE"));
    assert!(pass("N2ns6WidgetINS_5PlainEEE"));
    assert!(pass("N2ns6WidgetInEE"), "__int128 is fundamental");
    assert!(!pass("N2ns6WidgetIDv4_iEE"), "a vector type is none of clause 3's forms");
    // Clause 3: values.
    assert!(pass("N2ns7ByColorILNS_5ColorE1EEE"));
    assert!(pass("N2ns4IntsIJLi1ELi2ELi3EEEE"));
    assert!(pass("N2ns5TypesIJiNS_6GadgetE6GlobalEEE"));
    assert!(!pass("N2ns5TypesIJiN12_GLOBAL__N_14ImplEEEE"), "a pack with a failing element");
    assert!(pass("N2ns4NullILDn0EEE"));
    assert!(pass("N2ns3BigILxn5EEE"));
    assert!(!pass("N2ns3FltILd3ff8000000000000EEE"), "a floating-point value");
    assert!(!pass("N2ns3PtrIXcvPKiadL_ZNS_6globalEEEEE"), "a pointer to an entity (X18)");
    assert!(!pass("N2ns8TempTempISt8optionalEE"), "a template template argument");
    // Enclosing class templates' arguments.
    assert!(pass("N2ns5OuterIiE5InnerE"));
    assert!(!pass("N2ns5OuterIN12_GLOBAL__N_14ImplEE5InnerE"));
    // Enumerations.
    assert!(pass("N2ns5ColorE"));
    assert!(!pass("N12_GLOBAL__N_11EE"));
}

#[test]
fn cross_check_with_the_encoding() {
    let tu = parse(CORPUS);
    let v = verdicts(&tu.ast);
    let mut misses: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let (mut passing, mut tu_local) = (0, 0);
    for (enc, &(ok, kind, unnamed)) in &v {
        match per_tu_component(enc) {
            Some(c) => {
                assert!(!ok, "{enc} has a per-TU component ({c}) but passes");
                tu_local += 1;
            }
            None if ok => passing += 1,
            None => {
                let why = if unnamed {
                    "unnamed (typedef name for linkage)".to_owned()
                } else {
                    format!("{kind:?} with a failing argument or attribute")
                };
                misses.entry(why).or_default().push(enc);
            }
        }
    }
    println!("types: {passing} passing, {tu_local} with a per-TU component, misses (X18):");
    for (why, encs) in &misses {
        println!("  {why}: {} {encs:?}", encs.len());
    }
    assert!(passing >= 25 && tu_local >= 10, "the corpus exercises both sides");
}

#[test]
fn the_oracle_reads_encodings() {
    assert_eq!(per_tu_component("St8optionalIiE"), None);
    assert_eq!(per_tu_component("N12_GLOBAL__N_14ImplE"), Some("_GLOBAL__N"));
    assert_eq!(per_tu_component("ZL7local_tvE5Local"), Some("Z...E"));
    assert_eq!(per_tu_component("N1SUt_E"), Some("Ut/Ul"));
    assert_eq!(per_tu_component("3$_0"), Some("$_N"));
    assert_eq!(per_tu_component("5Utils"), None);
    assert_eq!(per_tu_component("N2ns4IntsIJLi1ELi2EEEE"), None);
    assert_eq!(per_tu_component("4ZooK"), None);
    assert_eq!(per_tu_component("N2ns3PtrIXadL_ZNS_6globalEEEEE"), None);
}
