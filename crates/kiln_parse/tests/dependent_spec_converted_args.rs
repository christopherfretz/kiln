//! A dependent class template-id's non-dependent value arguments are
//! converted to their parameters' types before the type is identified
//! ([temp.type]/1, WP-3321): `It<T, 1 == 1>`, `It<T, true>` and
//! `struct It<T, !false>` name one canonical type in the pattern, and
//! so do `V<T, 2>` and `V<T, 2u>` for an `unsigned` parameter, an
//! enumerator and its cast value, `nullptr` and `(int *)0`, and equal
//! class-type values. An argument for a parameter whose type depends on
//! a dependent argument stays an expression: `W<T, 1>` and `W<T, 1u>`
//! for `template <class U, U v>` are two types in the pattern and one in
//! the instantiation, as in clang. Reduced from real code (a tooling
//! client identifies entities by canonical type). Every source here
//! compiles with clang++-19 and clang++-22.

use std::sync::Arc;

use kiln_ast::Ast;
use kiln_ast::decl::DeclKind;
use kiln_ast::ids::{DeclId, TypeId};
use kiln_diag::{DiagEngine, DiagOptions, Level, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Parses `src` (no errors allowed) and runs `check` on the unit.
fn parse<R>(src: &str, check: impl FnOnce(&Ast) -> R) -> R {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let opts = ParseOptions { std: kiln_lex::Std::Cxx26, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let errors: Vec<String> =
        sink.take().iter().filter(|d| d.level >= Level::Error).map(|d| d.message.clone()).collect();
    assert!(errors.is_empty(), "{errors:?}\n{src}");
    check(&tu.ast)
}

/// The definitions of class `name` (the pattern first, then each
/// specialization).
fn classes(ast: &Ast, name: &str) -> Vec<DeclId> {
    let mut out: Vec<(bool, DeclId)> = ast
        .decls
        .ids()
        .filter(|&d| {
            ast.decls.get(d).kind() == DeclKind::CXXRecord
                && ast.definition(d) == Some(d)
                && ast
                    .decls
                    .get(d)
                    .name()
                    .as_ident()
                    .is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .map(|d| (ast.spec_info(d).is_some(), d))
        .collect();
    out.sort();
    out.into_iter().map(|(_, d)| d).collect()
}

/// The canonical pointee type of each pointer data member of `c`, in
/// member order.
fn pointees(ast: &Ast, c: DeclId) -> Vec<TypeId> {
    ast.decls
        .members(c)
        .filter(|&m| ast.decls.get(m).kind() == DeclKind::Field)
        .map(|m| {
            let ty = ast.decls.get(m).ty().expect("a field type");
            ast.types.canonical(ast.types.pointee(ty).expect("a pointer member")).ty()
        })
        .collect()
}

/// Class `C`'s pattern and its `specs` instantiations: in each, the
/// pointer members are grouped as `groups` says (the members of one
/// group point to one type, two groups to two types).
fn assert_groups(src: &str, specs: usize, pattern: &[usize], inst: &[usize]) {
    parse(src, |ast| {
        let cs = classes(ast, "C");
        assert_eq!(cs.len(), specs + 1, "C's pattern and specializations\n{src}");
        for (i, &c) in cs.iter().enumerate() {
            let groups = if i == 0 { pattern } else { inst };
            let ts = pointees(ast, c);
            assert_eq!(ts.len(), groups.iter().sum::<usize>(), "members of C #{i}\n{src}");
            let mut at = 0;
            let mut seen: Vec<TypeId> = Vec::new();
            for &n in groups {
                let first = ts[at];
                for (k, t) in ts[at..at + n].iter().enumerate() {
                    assert_eq!(*t, first, "C #{i}: member {} vs {at}\n{src}", at + k);
                }
                assert!(!seen.contains(&first), "C #{i}: group at {at} is an earlier type\n{src}");
                seen.push(first);
                at += n;
            }
        }
    });
}

#[test]
fn bool_argument_spellings() {
    let src = "template <class T, bool B> struct It {};\n\
               template <class T> struct C {\n\
                 It<T, true> *a; It<T, 1 == 1> *b; struct It<T, !false> *c; class It<T, 1> *d;\n\
                 It<T, false> *e; It<T, 0> *f;\n\
               };\n\
               C<int> c1; C<long> c2;\n";
    assert_groups(src, 2, &[4, 2], &[4, 2]);
}

#[test]
fn integral_and_enumeration_arguments() {
    let src = "enum E { e0, e1 };\n\
               enum class F : int { a, b };\n\
               template <class T, unsigned U> struct V {};\n\
               template <class T, long N> struct L {};\n\
               template <class T, E e> struct A {};\n\
               template <class T, F f> struct B {};\n\
               template <class T> struct C {\n\
                 V<T, 2> *a; V<T, 2u> *b; V<T, 1 + 1> *c;\n\
                 L<T, 97> *d; L<T, 'a'> *e;\n\
                 A<T, e1> *f; A<T, E(1)> *g;\n\
                 B<T, F::b> *h; B<T, F(1)> *i;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[3, 2, 2, 2], &[3, 2, 2, 2]);
}

#[test]
fn pointer_and_class_arguments() {
    let src = "int x;\n\
               struct P { int a; };\n\
               template <class T, int *p> struct Ptr {};\n\
               template <class T, int &r> struct Ref {};\n\
               template <class T, P v> struct G {};\n\
               template <class T> struct C {\n\
                 Ptr<T, nullptr> *a; Ptr<T, (int *)0> *b;\n\
                 Ptr<T, &x> *c; Ptr<T, (&x)> *d;\n\
                 Ref<T, x> *e; Ref<T, (x)> *f;\n\
                 G<T, P{1}> *g; G<T, P{0 + 1}> *h;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[2, 2, 2, 2], &[2, 2, 2, 2]);
}

#[test]
fn defaults_and_nesting() {
    let src = "template <class T, bool B> struct It {};\n\
               template <class T, long N = 1> struct Q {};\n\
               template <class T> struct C {\n\
                 Q<T> *a; Q<T, 1u> *b; Q<T, true> *c;\n\
                 It<It<T, 1 == 1>, true> *d; It<It<T, true>, 1> *e;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[3, 2], &[3, 2]);
}

#[test]
fn friend_elaborated_converted() {
    // The elaborated spelling is converted as the simple one is, so a
    // friend `class It<T, 1 == 1>` and a member `It<T, true> *` agree.
    let src = "template <class T, bool B> struct It {};\n\
               template <class T> struct C {\n\
                 friend class It<T, 1 == 1>;\n\
                 It<T, true> *a; struct It<T, 1 == 1> *b;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[2], &[2]);
}

#[test]
fn dependent_parameter_type_stays_expression() {
    // `U` is the dependent `T`: the arguments cannot be converted in the
    // pattern ([temp.over.link]/5 compares them as written), and are one
    // value once `T` is known.
    let src = "template <class U, U v> struct W {};\n\
               template <class T> struct C {\n\
                 W<T, 1> *a; W<T, 1u> *b;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[1, 1], &[2]);
}

#[test]
fn partial_specialization_current_instantiation() {
    // Inside the partial specialization `It<T, true>`, each spelling
    // names its current instantiation.
    let src = "template <class T, bool B> struct It {};\n\
               template <class T> struct It<T, true> {\n\
                 struct C {\n\
                   It *a; It<T, true> *b; It<T, 1 == 1> *c;\n\
                 };\n\
               };\n\
               It<int, true>::C c1;\n";
    assert_groups(src, 1, &[3], &[3]);
}

#[test]
fn pack_expansion_and_non_expansion_before_pack() {
    // WP-3689: a non-expansion value argument for a non-pack parameter is
    // converted even when a pack expansion or parameter pack follows.
    let src = "namespace n { constexpr int k = 3; }\n\
               template <int, int...> struct V {};\n\
               template <int, int, int...> struct V2 {};\n\
               template <int... Ns> struct C {\n\
                 V<n::k, Ns...> *a; V<3, Ns...> *b; V<1 == 1 ? 3 : 0, Ns...> *c;\n\
                 V<0, Ns...> *d;\n\
                 V2<n::k, Ns..., 4> *e; V2<3, Ns..., 4> *f;\n\
               };\n\
               C<1, 2> c1;\n";
    assert_groups(src, 1, &[3, 1, 2], &[3, 1, 2]);
}

#[test]
fn parameter_pack_element_conversions() {
    // WP-3689: non-expansion arguments matched to a parameter pack of known
    // element type convert (int, unsigned, enum, structural class, and
    // defaults before a pack).
    let src = "namespace n { constexpr int k = 3; }\n\
               enum E { e0 = 0, e2 = 2 };\n\
               struct S { int x; };\n\
               constexpr S s{1};\n\
               template <class T, int... Ns> struct W {};\n\
               template <class T, unsigned... Ns> struct WU {};\n\
               template <class T, E... Es> struct WE {};\n\
               template <class T, S... Ss> struct WS {};\n\
               template <class T, int = 1, int...> struct D2 {};\n\
               template <class T> struct C {\n\
                 W<T, n::k> *a; W<T, 3> *b; W<T, 1 + 2> *c; struct W<T, 3> *d;\n\
                 WU<T, 2> *e; WU<T, 2u> *f; WU<T, 1 + 1> *g;\n\
                 W<T, e2> *h; W<T, 2> *i;\n\
                 WE<T, e2> *j; WE<T, (E)2> *k;\n\
                 WS<T, S{1}> *l; WS<T, s> *m; WS<T, {1}> *n;\n\
                 D2<T> *o; D2<T, 1> *p; D2<T, 1u> *q;\n\
                 W<T> *r;\n\
               };\n\
               C<int> c1;\n";
    assert_groups(src, 1, &[4, 3, 2, 2, 3, 3, 1], &[4, 3, 2, 2, 3, 3, 1]);
}

#[test]
fn pack_expansion_inside_pack_vs_non_pack() {
    // WP-3689: once inside a trailing parameter pack, non-expansion arguments
    // after a pack expansion are still converted; after a pack expansion that
    // hits a non-pack parameter, subsequent arguments stay unconverted.
    let src = "namespace n { constexpr int k = 3; }\n\
               template <class T, int... Ns> struct W {};\n\
               template <class T, unsigned... Ns> struct WU {};\n\
               template <int, int> struct P2 {};\n\
               template <int, int, int...> struct V2 {};\n\
               template <class T, int... Ns> struct C {\n\
                 W<T, Ns..., n::k> *a; W<T, Ns..., 3> *b;\n\
                 WU<T, Ns..., 2> *c; WU<T, Ns..., 2u> *d;\n\
                 P2<Ns..., n::k> *e; P2<Ns..., 3> *f;\n\
                 V2<3, Ns..., 1 + 1> *g; V2<3, Ns..., 2> *h;\n\
               };\n\
               C<int, 1> c1;\n";
    assert_groups(src, 1, &[2, 2, 1, 1, 1, 1], &[2, 2, 2, 2]);
}

#[test]
fn auto_and_dependent_pack_elements() {
    // WP-3689: `auto...` converts each element to its deduced type (`n::k` and
    // `3` are both `int(3)`, `1` and `1u` differ); `template<class U, U...>`
    // keeps expressions when `U` is dependent (`WD<T, 1>` vs `WD<T, 1u>`) and
    // converts when `U` is concrete (`WD<int, N, 1>` vs `WD<int, N, 1u>`).
    let src = "namespace n { constexpr int k = 3; }\n\
               template <class T, auto... Ns> struct WA {};\n\
               template <class U, U... vs> struct WD {};\n\
               template <class T, int N, int... Ns> struct C {\n\
                 WA<T, n::k> *a; WA<T, 3> *b;\n\
                 WA<T, 1> *c; WA<T, 1u> *d;\n\
                 WA<T, Ns..., n::k> *e; WA<T, Ns..., 3> *f;\n\
                 WD<T, 1> *g; WD<T, 1u> *h;\n\
                 WD<int, N, 1> *i; WD<int, N, 1u> *j;\n\
               };\n\
               C<int, 5, 7> c1;\n";
    assert_groups(src, 1, &[2, 1, 1, 2, 1, 1, 2], &[2, 1, 1, 2, 2, 2]);
}

#[test]
fn partial_specialization_with_pack_current_instantiation() {
    // Inside `W<T, 3>`, `W`, `W<T, 3>`, and `W<T, 1 + 2>` name one type, and
    // out-of-line member definitions agree with in-class return types.
    let src = "template <class T, int... Ns> struct W {\n\
                 static W *self();\n\
               };\n\
               template <class T, int... Ns> W<T, Ns...> *W<T, Ns...>::self() { return nullptr; }\n\
               template <class T> struct W<T, 3> {\n\
                 static W *make();\n\
                 struct C {\n\
                   W *a; W<T, 3> *b; W<T, 1 + 2> *c;\n\
                 };\n\
               };\n\
               template <class T> W<T, 3> *W<T, 1 + 2>::make() { return nullptr; }\n\
               W<int, 3>::C c1;\n";
    assert_groups(src, 1, &[3], &[3]);
}
