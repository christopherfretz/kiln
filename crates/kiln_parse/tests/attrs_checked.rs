//! The checked marker of attribute nodes (spec/ast.md §7, spec/sema.md
//! §4.5; WP-2022): once the translation unit is done, every attribute
//! attached to a declaration or an attributed statement has been checked
//! against what it appertains to ([dcl.attr.grammar]/5), enumerators
//! ([dcl.enum]/2) and lambda call operators ([expr.prim.lambda.closure]/7)
//! included, and says so. A tooling client reading the AST after the
//! translation unit relies on it: a checked `cleanup` holds the function's
//! `DeclId`, not the argument's `ExprId`.

use std::sync::Arc;

use kiln_ast::attr::{AttrArena, AttrKind, AttrNode, AttrSyntax};
use kiln_ast::decl::DeclKind as K;
use kiln_ast::ids::RawId;
use kiln_ast::stmt::StmtKind;
use kiln_ast::{Ast, AttrId, DeclId, StmtId};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// Attributes on every kind of declaration sema attaches them to, the
/// pushed `#pragma clang attribute` ones, the implicit ones sema adds
/// (a known library function's), template patterns and their
/// instantiations, and attributed statements. Some are invalid where
/// written and are dropped.
const SRC: &str = r#"
extern "C" int printf(const char *, ...);
extern "C" void *malloc(decltype(sizeof 0));
void done(int *);
[[deprecated("d")]] int old_var;
__attribute__((visibility("hidden"))) int hidden_var;
__thread int tls_var __attribute__((tls_model("initial-exec")));
__attribute__((constructor(200))) void ctor_fn();
__attribute__((destructor)) void dtor_fn();
void nn(int *, int *) __attribute__((nonnull(2)));
void cb(void (*)(int), int) __attribute__((callback(1, 2)));
[[nodiscard]] int must();
using Alias [[deprecated]] = int;
typedef int Td __attribute__((aligned(8)));
namespace [[deprecated]] ns { int in_ns; }
namespace __attribute__((visibility("default"))) vis_ns { int in_vis; }
struct [[nodiscard]] __attribute__((aligned(16))) S {
  [[deprecated]] int field;
  __attribute__((noinline)) void method(int *p __attribute__((nonnull))) {}
  friend void friend_fn() __attribute__((noinline)) {}
  S() asm("s_ctor");
  operator int() asm("s_conv");
};
extern int labelled asm("labelled_var");
#pragma weak weak_target
extern "C" void weak_target();
#pragma GCC visibility push(hidden)
int pragma_hidden;
#pragma GCC visibility pop
enum E { a [[deprecated("x")]], b __attribute__((deprecated)), c [[maybe_unused]], d [[nodiscard]], e alignas(8) };
enum class Sc : int { x [[maybe_unused]] = 1 };
#pragma clang attribute push(__attribute__((annotate("pushed"))), apply_to = any(function, variable, enum_constant))
int pushed_var;
void pushed_fn();
enum Pushed { pushed_enumerator };
#pragma clang attribute pop
template <class T> struct Tm {
  enum F { p [[deprecated]] = sizeof(T), q [[nodiscard]] };
  [[gnu::noinline]] T get() { return T(); }
};
template struct Tm<int>;
template <class T> [[gnu::cold]] T tf(T t) { return t; }
int (*tfp)(int) = &tf<int>;
template <class T> T tcl() { T w __attribute__((cleanup(done))) = 0; [[assume(w == 0)]]; return w; }
int tcl_int = tcl<int>();
template <int N> __attribute__((constructor(N))) void tctor() {}
template void tctor<300>();
template <class T> struct G { G(T) {} };
G(int) -> G<int>;
int run(int k) {
  int v __attribute__((cleanup(done))) = 1;
  auto l1 = [] [[nodiscard]] () { return 1; };
  auto l2 = [] (int *p) __attribute__((nonnull(2), cold)) { return *p; };
  auto l3 = [] [[deprecated]] (int) __attribute__((alloc_size(2))) -> void * { return nullptr; };
  auto l4 = [] (int) __attribute__((alloc_size(1))) { return (void *)0; };
  auto [s1, s2] [[maybe_unused]] = S{};
  try { throw 1; } catch ([[maybe_unused]] int caught) {}
  switch (k) {
  case 0:
    k += 1;
    [[fallthrough]];
  case 1:
    [[likely]] return v + l1() + l2(&k);
  }
  [[assume(k > 2)]];
  return printf("%d", k) + (malloc(4) != nullptr);
}
"#;

fn parse(src: &str) -> (Ast, u32) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let target = Target::for_arch(Arch::X86_64);
    let opts = ParseOptions::default();
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    (tu.ast, engine.error_count())
}

/// Every attribute reachable from the AST: each declaration's list and
/// each attributed statement's.
fn reachable(ast: &Ast) -> Vec<(Option<DeclId>, AttrId)> {
    let mut out = Vec::new();
    for d in ast.decls.ids() {
        out.extend(ast.lists.iter::<AttrId>(ast.attrs.of_decl(d)).map(|a| (Some(d), a)));
    }
    for i in 1..=ast.stmts.len() {
        let s = StmtId::new(u32::try_from(i).expect("statement index"));
        if ast.stmts.get(s).kind() == StmtKind::AttributedStmt {
            out.extend(ast.lists.iter::<AttrId>(ast.stmts.stmt_attrs(s)).map(|a| (None, a)));
        }
    }
    out
}

/// [dcl.attr.grammar]/5: after the translation unit every attribute
/// attached to a declaration or a statement is checked, the implicit and
/// the pushed ones too, in templates and their instantiations.
#[test]
fn every_reachable_attribute_is_checked_dcl_attr_grammar_5() {
    let (ast, errors) = parse(SRC);
    assert!(errors > 0, "the invalid attributes are diagnosed");
    let all = reachable(&ast);
    assert!(all.len() > 30, "the source attaches many attributes ({})", all.len());
    let unchecked: Vec<String> = all
        .iter()
        .filter(|&&(_, a)| !ast.attrs.is_checked(a))
        .map(|&(d, a)| {
            let n = ast.attrs.get(a);
            let on = d.map_or_else(
                || "a statement".to_owned(),
                |d| format!("{:?}", ast.decls.get(d).kind()),
            );
            format!("kind {} flags {:#x} on {on}", n.kind.0, n.flags)
        })
        .collect();
    assert!(unchecked.is_empty(), "unchecked attributes:\n{}", unchecked.join("\n"));
    let has = |k: AttrKind| all.iter().any(|&(_, a)| ast.attrs.get(a).kind == k);
    for k in [
        AttrKind::CLEANUP,
        AttrKind::VISIBILITY,
        AttrKind::TLS_MODEL,
        AttrKind::CONSTRUCTOR,
        AttrKind::DESTRUCTOR,
        AttrKind::NONNULL,
        AttrKind::CALLBACK,
        AttrKind::DEPRECATED,
        AttrKind::ANNOTATE,
        AttrKind::ALLOC_SIZE,
        AttrKind::FALLTHROUGH,
        AttrKind::ASSUME,
        AttrKind::ASM_LABEL,
        AttrKind::WEAK,
    ] {
        assert!(has(k), "the source attaches a kind {} attribute", k.0);
    }
    let implicit = all.iter().filter(|&&(_, a)| ast.attrs.get(a).flags & AttrNode::IMPLICIT != 0);
    assert!(implicit.count() >= 2, "`#pragma weak` and `#pragma GCC visibility` add implicit ones");
}

/// The checked `cleanup` of `v` names the function: its `a` is the
/// `DeclId` of `done`, which a client can only trust on a checked node.
#[test]
fn checked_cleanup_holds_the_function_dcl_attr_grammar_5() {
    let (ast, _) = parse(SRC);
    let cleanup = reachable(&ast)
        .into_iter()
        .map(|(_, a)| *ast.attrs.get(a))
        .find(|n| n.kind == AttrKind::CLEANUP)
        .expect("v's cleanup");
    assert!(cleanup.is_checked());
    let f = DeclId::from_raw(cleanup.a).expect("a DeclId");
    assert!(ast.decls.get(f).kind().is_function());
}

/// [dcl.enum]/2, [expr.prim.lambda.closure]/7: an enumerator's and a
/// lambda call operator's attributes are checked against them, so one
/// whose subject is wrong (`nodiscard` and `alignas` on an enumerator,
/// `alloc_size` on a call operator with a deduced return type) is
/// dropped, in a template's enumeration too.
#[test]
fn enumerator_and_call_operator_attributes_are_checked_dcl_enum_2() {
    let (ast, _) = parse(SRC);
    let all = reachable(&ast);
    let on = |k: AttrKind, dk: K| {
        all.iter()
            .filter(|&&(d, a)| {
                ast.attrs.get(a).kind == k && d.is_some_and(|d| ast.decls.get(d).kind() == dk)
            })
            .count()
    };
    assert_eq!(on(AttrKind::NODISCARD, K::EnumConstant), 0);
    assert_eq!(on(AttrKind::ALIGNED, K::EnumConstant), 0);
    // `a`, `b` and the template's `p`.
    assert!(on(AttrKind::DEPRECATED, K::EnumConstant) >= 3);
    assert!(on(AttrKind::ANNOTATE, K::EnumConstant) >= 1, "the pushed annotate");
    // `l3`'s, not `l4`'s.
    assert_eq!(on(AttrKind::ALLOC_SIZE, K::CXXMethod), 1);
}

/// A node as written is not checked until sema marks it; an implicit one
/// is built checked.
#[test]
fn marker_is_set_by_checking_or_by_implicit_construction() {
    let mut arena = AttrArena::new();
    let node = AttrNode {
        kind: AttrKind::DEPRECATED,
        syntax: AttrSyntax::Cxx11,
        flags: 0,
        loc: kiln_source::Loc::INVALID,
        a: 0,
        b: 0,
    };
    let written = arena.push(node);
    assert!(!arena.is_checked(written));
    arena.mark_checked(&[written]);
    assert!(arena.is_checked(written));
    let implicit = arena.push(AttrNode { flags: AttrNode::IMPLICIT, ..node });
    assert!(arena.get(implicit).is_checked());
}
