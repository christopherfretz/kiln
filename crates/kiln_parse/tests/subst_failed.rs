//! Silent failures (spec/ast.md §4.1 "Silent failures", spec/sema-templates.md
//! §5.7.3; WP-3281, WP-3282): `DeclArena::subst_failed` marks a declaration
//! kiln made invalid while every error raised forming it was trapped, with
//! what failed to form, so that in a translation unit without errors every
//! invalid declaration is marked. Each case also runs the checker
//! (`ParseOptions::verify_subst_failed`), which reports an unmarked invalid
//! declaration, a declaration a failed formation left without an owner, and
//! a recorded reference to a specialization that was not formed.

use std::sync::Arc;

use kiln_ast::decl::{DeclFlags, DeclKind, SubstFailedKind};
use kiln_ast::templates::SpecKind;
use kiln_ast::{Ast, DeclId, QualType};
use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_parse::{ConstexprLimits, ParseOptions, parse_tu};
use kiln_sema::target::{Arch, Target};
use kiln_source::SourceManager;
use kiln_source::symbol::Interner;

/// The AST of `src` (C++26 at clang compat `compat`, the checker on) and
/// its diagnostics' messages.
fn compile_at(src: &str, compat: u32) -> (Ast, Vec<String>) {
    let sm = SourceManager::new();
    let file = sm.add_virtual("t.cpp", src.to_owned()).expect("location space");
    let interner = Arc::new(Interner::new());
    let sink = MemorySink::new();
    let dopts = DiagOptions { clang_compat: compat, ..DiagOptions::default() };
    let engine = DiagEngine::new(&sm, &dopts, sink.clone());
    let opts =
        ParseOptions { clang_compat: compat, verify_subst_failed: true, ..ParseOptions::default() };
    let target = Target::for_arch(Arch::X86_64);
    let tu = parse_tu(&sm, &interner, file, &engine, target, &opts, ConstexprLimits::default());
    let diags = sink.take().into_iter().map(|d| d.message).collect();
    (tu.ast, diags)
}

/// [`compile_at`] at the default compat; asserts no diagnostic at all
/// (the checker's findings are errors).
#[track_caller]
fn clean(src: &str) -> Ast {
    let (ast, diags) = compile_at(src, 22);
    assert!(diags.is_empty(), "unexpected diagnostics:\n{}", diags.join("\n"));
    ast
}

/// Every declaration of kind `kind` named `name` (`Var` also takes
/// `VarTemplateSpecialization`).
fn named(ast: &Ast, kind: DeclKind, name: &str) -> Vec<DeclId> {
    (1..=u32::try_from(ast.decls.len()).unwrap())
        .map(DeclId::new)
        .filter(|&d| {
            let n = ast.decls.get(d);
            let k = n.kind();
            (k == kind || kind == DeclKind::Var && k == DeclKind::VarTemplateSpecialization)
                && n.name().as_ident().is_some_and(|s| ast.symbols().as_str(s) == name)
        })
        .collect()
}

/// The specializations of kind `kind` named `name` (those with spec info).
fn specs(ast: &Ast, kind: DeclKind, name: &str) -> Vec<DeclId> {
    named(ast, kind, name).into_iter().filter(|&d| ast.spec_info(d).is_some()).collect()
}

/// The marked declarations as `(kind, mark)`.
fn marks(ast: &Ast) -> Vec<(DeclKind, SubstFailedKind)> {
    ast.decls.subst_failed_decls().into_iter().map(|(d, k)| (ast.decls.get(d).kind(), k)).collect()
}

fn nparams(ast: &Ast, f: DeclId) -> usize {
    ast.decls.function(f).map_or(0, |f| ast.lists.len(f.params))
}

fn invalid(ast: &Ast, d: DeclId) -> bool {
    ast.decls.get(d).has(DeclFlags::INVALID)
}

/// Kind 1 ([temp.deduct.general]/8), reduced from real code: substitution into
/// the trailing return type fails: the specialization has the error type
/// and no parameters, and nothing names it.
#[test]
fn kind1_signature_substitution_fails_temp_deduct_general_8() {
    let ast = clean(
        "template <class T> auto g(T t) -> decltype(t.size());\n\
         int g(...);\n\
         int u() { return g(1); }\n",
    );
    let s = specs(&ast, DeclKind::Function, "g");
    assert_eq!(s.len(), 1);
    assert!(invalid(&ast, s[0]));
    assert_eq!(ast.decls.subst_failed(s[0]), Some(SubstFailedKind::FunctionSignature));
    assert_eq!(ast.decls.get(s[0]).ty(), Some(QualType::ERROR));
    assert_eq!(nparams(&ast, s[0]), 0);
    // The parameter substituted before the failure keeps the specialization
    // as its owner (rule A).
    let after: Vec<_> =
        named(&ast, DeclKind::ParmVar, "t").into_iter().filter(|&p| p > s[0]).collect();
    assert_eq!(after.len(), 1);
    assert_eq!(ast.decls.get(after[0]).ctx(), Some(s[0]));
    assert_eq!(marks(&ast), [(DeclKind::Function, SubstFailedKind::FunctionSignature)]);
}

/// Kind 1 with a signature that substitutes (`Ok`) while the frame traps
/// an error: a narrowing in `decltype` ([dcl.init.list]/3.9,
/// [temp.deduct.general]/8). The specialization keeps its type and
/// parameters.
#[test]
fn kind1_trapped_narrowing_keeps_the_signature_dcl_init_list_3() {
    let ast = clean(
        "template <class T> auto h(T t) -> decltype(int{t});\n\
         long h(...);\n\
         long u() { return h(1.5); }\n",
    );
    let s = specs(&ast, DeclKind::Function, "h");
    assert_eq!(s.len(), 1);
    assert_eq!(ast.decls.subst_failed(s[0]), Some(SubstFailedKind::FunctionSignature));
    assert_ne!(ast.decls.get(s[0]).ty(), Some(QualType::ERROR));
    assert_eq!(nparams(&ast, s[0]), 1);
}

/// Kind 1 for a deduction guide ([over.match.class.deduct]/1): the user
/// guide fails to substitute, the implicit one deduces `A<int>`.
#[test]
fn kind1_deduction_guide_over_match_class_deduct_1() {
    let ast = clean(
        "template <class T> struct A { A(T) {} };\n\
         template <class T> A(T) -> A<typename T::type>;\n\
         A a(1);\n",
    );
    let failed: Vec<_> = ast
        .decls
        .subst_failed_decls()
        .into_iter()
        .filter(|&(d, _)| ast.decls.get(d).kind() == DeclKind::CXXDeductionGuide)
        .collect();
    assert_eq!(failed.len(), 1, "{:?}", marks(&ast));
    assert_eq!(failed[0].1, SubstFailedKind::FunctionSignature);
}

/// Kind 2 ([temp.deduct.general]/7): only the `explicit(expr)` fails; the
/// specialization has its full type.
#[test]
fn kind2_explicit_specifier_fails_temp_deduct_general_7() {
    let ast = clean(
        "struct S {\n\
           template <class T> explicit(T::value) S(T) {}\n\
           S(...) {}\n\
         };\n\
         S s = 1;\n",
    );
    let m = marks(&ast);
    assert_eq!(m, [(DeclKind::CXXConstructor, SubstFailedKind::ExplicitSpecifier)]);
    let (d, _) = ast.decls.subst_failed_decls()[0];
    assert_ne!(ast.decls.get(d).ty(), Some(QualType::ERROR));
}

/// Kind 3 ([temp.spec.partial.match]/1, [temp.deduct.general]/8): the
/// partial-specialization choice for `v<int, int>` is ambiguous inside a
/// deduction; the function specialization fails with it.
#[test]
fn kind3_variable_partial_ambiguity_temp_spec_partial_match_1() {
    let ast = clean(
        "template <class T, class U> constexpr int v = 0;\n\
         template <class T> constexpr int v<T, int> = 1;\n\
         template <class U> constexpr int v<int, U> = 2;\n\
         template <class T> auto f(T) -> decltype(v<T, int>);\n\
         long f(...);\n\
         long x = f(1);\n",
    );
    let mut m = marks(&ast);
    m.sort_by_key(|&(_, k)| k.value());
    assert_eq!(
        m,
        [
            (DeclKind::Function, SubstFailedKind::FunctionSignature),
            (DeclKind::Var, SubstFailedKind::VariablePartialAmbiguity),
        ]
    );
}

/// Not kind 3 (amendment B): a variable template specialization whose
/// template, or whose matching partial specialization, is only declared
/// is a valid nameable entity ([temp.inst]/7: defined elsewhere).
#[test]
fn declared_variable_template_spec_is_valid_temp_inst_7() {
    let (ast, diags) = compile_at(
        "template <class T> extern int v;\n\
         template <class T> extern int w;\n\
         template <class T> extern int w<T*>;\n\
         int* p = &v<int>;\n\
         int* q = &w<int*>;\n",
        22,
    );
    assert!(diags.iter().all(|d| d.contains("no definition is available")), "{diags:?}");
    for name in ["v", "w"] {
        let s = specs(&ast, DeclKind::Var, name);
        assert_eq!(s.len(), 1, "{name}");
        assert!(!invalid(&ast, s[0]), "{name}");
        assert_eq!(ast.decls.subst_failed(s[0]), None, "{name}");
    }
    assert!(marks(&ast).is_empty());
}

/// Kind 4 ([temp.spec.partial.match]/1): `X<int, int>` matches two partial
/// specializations, neither more specialized; inside a deduction the
/// error is trapped. The specialization is exactly a never-instantiated
/// one (no definition, members or bases) and stays nameable.
#[test]
fn kind4_class_partial_ambiguity_temp_spec_partial_match_1() {
    let ast = clean(
        "template <class T, class U> struct X {};\n\
         template <class T> struct X<T, int> { using t = int; };\n\
         template <class U> struct X<int, U> { using t = int; };\n\
         template <class T> auto f(T) -> typename X<T, int>::t;\n\
         long f(...);\n\
         long y = f(1);\n\
         X<int, int>* p = nullptr;\n",
    );
    let x: Vec<_> = specs(&ast, DeclKind::CXXRecord, "X");
    assert_eq!(x.len(), 1);
    let x = x[0];
    assert_eq!(ast.decls.subst_failed(x), Some(SubstFailedKind::ClassPartialAmbiguity));
    assert_eq!(ast.spec_info(x).map(|(_, i)| i.kind), Some(SpecKind::Undeclared));
    assert_eq!(ast.definition(x), None);
    assert_eq!(ast.decls.members(x).count(), 0);
    assert_eq!(ast.decls.bases(x, &ast.lists).count(), 0);
}

/// The same ambiguity outside a deduction is an error: invalid, unmarked.
#[test]
fn kind4_ambiguity_hard_is_diagnosed_and_unmarked() {
    let (ast, diags) = compile_at(
        "template <class T, class U> struct X {};\n\
         template <class T> struct X<T, int> {};\n\
         template <class U> struct X<int, U> {};\n\
         X<int, int> z;\n",
        22,
    );
    assert!(diags.iter().any(|d| d.contains("ambiguous partial specializations")), "{diags:?}");
    let x = specs(&ast, DeclKind::CXXRecord, "X");
    assert!(invalid(&ast, x[0]));
    assert_eq!(ast.decls.subst_failed(x[0]), None);
}

/// WP-3283 ([temp.spec.partial.match]/1, [temp.deduct.general]/8): an
/// ambiguity a deduction trapped first is reported by the first use
/// outside a trap, once (a second use stays quiet, as after a first
/// report), and the memo stays: the specialization keeps `INVALID` and its
/// mark, since the trap that made it invalid printed nothing. Kinds 3 and 4.
#[test]
fn kind3_kind4_trapped_ambiguity_reported_at_first_hard_use() {
    let (ast, diags) = compile_at(
        "template <class T, class U> struct X;\n\
         template <class T> struct X<T, int> {};\n\
         template <class U> struct X<int, U> {};\n\
         template <class T, class U> constexpr int v = 0;\n\
         template <class T> constexpr int v<T, int> = 1;\n\
         template <class U> constexpr int v<int, U> = 2;\n\
         template <class T> auto f(T) -> decltype(sizeof(X<T, int>));\n\
         long f(...);\n\
         template <class T> auto g(T) -> decltype(v<T, int>);\n\
         long g(...);\n\
         long y = f(1) + g(1);\n\
         X<int, int> a;\n\
         X<int, int> b;\n\
         int c = v<int, int>;\n\
         int d = v<int, int>;\n",
        22,
    );
    let ambiguous: Vec<_> =
        diags.iter().filter(|d| d.contains("ambiguous partial specializations")).collect();
    assert_eq!(ambiguous.len(), 2, "{diags:?}");
    assert!(ambiguous[0].contains("'X<int, int>'"), "{diags:?}");
    assert!(ambiguous[1].contains("'v<int, int>'"), "{diags:?}");
    let x = specs(&ast, DeclKind::CXXRecord, "X");
    assert_eq!(x.len(), 1);
    assert!(invalid(&ast, x[0]));
    assert_eq!(ast.decls.subst_failed(x[0]), Some(SubstFailedKind::ClassPartialAmbiguity));
    let v = specs(&ast, DeclKind::Var, "v");
    assert_eq!(v.len(), 1);
    assert!(invalid(&ast, v[0]));
    assert_eq!(ast.decls.subst_failed(v[0]), Some(SubstFailedKind::VariablePartialAmbiguity));
}

/// Kind 1 on a generic lambda's call operator whose trailing return type
/// holds a GNU statement-expression that fails ([temp.deduct.general]/8);
/// the local it declared and made invalid is kind 5, though it keeps the
/// pattern's context.
#[test]
fn statement_expression_in_failed_signature_belongs_to_it() {
    let ast = clean(
        "template <class L> constexpr bool callable = requires(L l) { l(1); };\n\
         void g() {\n\
           auto l = [](auto t) -> decltype(({ int x = t.foo(); x; })) { return 0; };\n\
           static_assert(!callable<decltype(l)>);\n\
         }\n",
    );
    assert_eq!(
        marks(&ast),
        [
            (DeclKind::CXXMethod, SubstFailedKind::FunctionSignature),
            (DeclKind::Var, SubstFailedKind::LocalOfFailedSignature),
        ]
    );
}

/// [temp.inst]/2 inside the pattern's definition: `basic<long>` cannot be
/// instantiated while `basic` is being defined, a substitution failure of
/// `f`; the class completes and instantiates normally afterwards, so it
/// carries neither INVALID nor a mark (the failure is the use's).
#[test]
fn class_used_in_its_own_definition_completes_later_temp_inst_2() {
    let ast = clean(
        "template <class T> auto f(T* p) -> decltype(sizeof(*p));\n\
         char f(...);\n\
         template <class S> struct basic {\n\
           static constexpr int k = sizeof(f((basic<long>*)nullptr));\n\
           int m;\n\
         };\n\
         basic<int> bi;\n\
         basic<long> bl;\n\
         static_assert(basic<int>::k == 1);\n",
    );
    for b in specs(&ast, DeclKind::CXXRecord, "basic") {
        assert!(!invalid(&ast, b), "{b:?}");
        assert!(ast.definition(b).is_some(), "{b:?} instantiated");
    }
    assert_eq!(marks(&ast), [(DeclKind::Function, SubstFailedKind::FunctionSignature)]);
}

/// The same use outside a deduction is an error, and the specialization is
/// invalid and unmarked, as before.
#[test]
fn class_used_in_its_own_definition_hard_is_diagnosed() {
    let (ast, diags) = compile_at(
        "template <class S> struct basic {\n\
           static constexpr int k = sizeof(basic<long>);\n\
         };\n\
         basic<int> bi;\n",
        22,
    );
    assert!(diags.iter().any(|d| d.contains("within its own definition")), "{diags:?}");
    let b: Vec<_> = specs(&ast, DeclKind::CXXRecord, "basic")
        .into_iter()
        .filter(|&b| invalid(&ast, b))
        .collect();
    assert_eq!(b.len(), 1);
    assert_eq!(ast.decls.subst_failed(b[0]), None);
}

/// Q4 ([temp.deduct.general]/5, [over.match.viable]/3): a candidate whose
/// associated constraints are not satisfied. At clang 22 no specialization
/// is formed; at clang 19 it is formed, valid and unmarked.
#[test]
fn constraint_dropped_candidate_is_neither_temp_deduct_general_5() {
    let src = "template <class T> requires (sizeof(T) > 100) void c(T);\n\
               void c(...);\n\
               void w() { c(1); }\n";
    let ast = clean(src);
    assert!(specs(&ast, DeclKind::Function, "c").is_empty());
    let (ast, diags) = compile_at(src, 19);
    assert!(diags.is_empty(), "{diags:?}");
    let s = specs(&ast, DeclKind::Function, "c");
    assert_eq!(s.len(), 1);
    assert!(!invalid(&ast, s[0]));
    assert_eq!(ast.decls.subst_failed(s[0]), None);
}

/// A class declared before the failed signature (`V<int>` named by the
/// call, `Al<int>` by its default argument) is completed while it is
/// substituted: its member template is listed through its template, and
/// the checker finds no orphan (reduced from a standard library's
/// `is_invocable` of a container).
#[test]
fn class_completed_in_failed_signature_lists_its_member_template() {
    let ast = clean(
        "template <class T> T&& declval();\n\
         template <class F> auto test(int) -> decltype(declval<F>()());\n\
         template <class F> void test(...);\n\
         template <class T> struct Al { Al(); template <class U> Al(const Al<U>&); };\n\
         template <class T, class A = Al<T>> struct V { A a; };\n\
         using X = decltype(test<V<int>>(0));\n",
    );
    for a in specs(&ast, DeclKind::CXXRecord, "Al") {
        assert!(!invalid(&ast, a), "{a:?}");
        assert!(ast.definition(a).is_some(), "{a:?} instantiated");
    }
    assert_eq!(marks(&ast), [(DeclKind::Function, SubstFailedKind::FunctionSignature)]);
}

/// The same class befriends a class template: completing it substitutes
/// the friend's head only to compare it with the template's (clang
/// `CheckTemplateParameterList`), and that head's parameter is the one
/// template parameter no declaration holds (spec/ast.md §4.1 rule A).
#[test]
fn friend_class_template_head_in_failed_signature() {
    let src = "template <class T> T&& declval();\n\
               template <class F> auto test(int) -> decltype(declval<F>()());\n\
               template <class F> void test(...);\n\
               template <class U> struct Fr;\n\
               template <class T> struct Al { template <class U> friend struct Fr; };\n\
               template <class T, class A = Al<T>> struct V { A a; };\n\
               using X = decltype(test<V<int>>(0));\n";
    let ast = clean(src);
    assert_eq!(marks(&ast), [(DeclKind::Function, SubstFailedKind::FunctionSignature)]);
}

/// WP-3282 ([dcl.fct.def.coroutine]/5): the promise is not constructible
/// from the parameter copies, so it is default-constructed; the trial
/// leaves no second `__promise` and nothing invalid.
#[test]
fn coroutine_promise_trial_leaves_nothing_dcl_fct_def_coroutine_5() {
    let ast = clean(
        "namespace std {\n\
         template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; };\n\
         template <class P = void> struct coroutine_handle {\n\
           static coroutine_handle from_address(void*) noexcept { return {}; }\n\
           void* address() const noexcept { return nullptr; }\n\
         };\n\
         template <> struct coroutine_handle<void> {\n\
           template <class P> coroutine_handle(coroutine_handle<P>) noexcept {}\n\
           coroutine_handle() = default;\n\
           static coroutine_handle from_address(void*) noexcept { return {}; }\n\
         };\n\
         struct suspend_never {\n\
           bool await_ready() const noexcept { return true; }\n\
           void await_suspend(coroutine_handle<>) const noexcept {}\n\
           void await_resume() const noexcept {}\n\
         };\n\
         }\n\
         struct task {\n\
           struct promise_type {\n\
             promise_type(int, int) {}\n\
             promise_type() {}\n\
             task get_return_object() { return {}; }\n\
             std::suspend_never initial_suspend() { return {}; }\n\
             std::suspend_never final_suspend() noexcept { return {}; }\n\
             void return_void() {}\n\
             void unhandled_exception() {}\n\
           };\n\
         };\n\
         task co(double) { co_return; }\n",
    );
    assert_eq!(named(&ast, DeclKind::Var, "__promise").len(), 1);
    let any_invalid =
        (1..=u32::try_from(ast.decls.len()).unwrap()).any(|i| invalid(&ast, DeclId::new(i)));
    assert!(!any_invalid);
}

/// [expr.prim.req.general]/5: `this` named in a requires-expression inside
/// a lambda that captures nothing makes the requirement fail, not the
/// lambda: its call operator stays valid.
#[test]
fn this_in_requires_expression_keeps_the_lambda_expr_prim_req_general_5() {
    let ast = clean(
        "struct C {\n\
           int m;\n\
           void f() {\n\
             auto l = [] { return requires { this->m; }; };\n\
             (void)l;\n\
           }\n\
         };\n",
    );
    for op in named(&ast, DeclKind::CXXMethod, "operator()") {
        assert!(!invalid(&ast, op));
    }
}

/// WP-3685 ([except.spec]/7, [temp.deduct.general]/8): evaluating a special
/// member's implicit exception specification (`SynthKind::ExceptionSpec`,
/// clang `ExceptionSpecEvaluation`) does not break an enclosing SFINAE
/// context. When a class inheriting a constructor template with defaulted
/// parameters triggers `__is_constructible(Data)` on a nested class inside
/// the enclosing class definition, the unavailable default member
/// initializers are a substitution failure, not a hard error, and the
/// fields stay valid.
#[test]
fn exception_spec_eval_is_transparent_to_sfinae_wp3685() {
    let ast = clean(
        "template <bool B, class T = void> struct enable_if {};\n\
         template <class T> struct enable_if<true, T> { using type = T; };\n\
         template <class T> struct is_dc { static constexpr bool value = __is_constructible(T); };\n\
         template <class T1, class T2> struct pair {\n\
           T1 first; T2 second;\n\
           template <class U1 = T1, class U2 = T2,\n\
                     typename enable_if<is_dc<U1>::value && is_dc<U2>::value, bool>::type = true>\n\
           constexpr pair() : first(), second() {}\n\
         };\n\
         template <class K, class V> struct DP : pair<K, V> { using pair<K, V>::pair; };\n\
         template <class K, class V> struct Map { char s[sizeof(DP<K, V>)]; };\n\
         struct M {\n\
           struct Data { bool H = false; unsigned ID = 0; };\n\
           Map<const int *, Data> Info;\n\
         };\n\
         struct N {\n\
           struct Inner { int x = 1; };\n\
           static constexpr bool ok = __is_constructible(Inner);\n\
           static_assert(!ok);\n\
         };\n\
         int main() { M m; M::Data d; N::Inner i; return d.H + d.ID + i.x - 1; }\n",
    );
    for fname in ["H", "ID", "x"] {
        let fields = named(&ast, DeclKind::Field, fname);
        assert_eq!(fields.len(), 1, "field {fname}");
        assert!(!invalid(&ast, fields[0]), "field {fname} stayed valid");
        assert_eq!(ast.decls.subst_failed(fields[0]), None);
    }
}

/// WP-3697: a class template specialization marked with kind 4
/// (`ClassPartialAmbiguity`) during a SFINAE probe has its `INVALID` flag
/// and kind-4 mark cleared when a later explicit specialization defines
/// it, and the retired failed function specialization is no longer
/// reachable from `find_spec` / `specs_of` once re-deduction succeeds.
#[test]
fn kind4_mark_cleared_by_later_explicit_specialization_wp3697() {
    let ast = clean(
        "template <class T, class U> struct X {};\n\
         template <class T> struct X<T, int> { using t = int; };\n\
         template <class U> struct X<int, U> { using t = int; };\n\
         template <class T> auto f(T) -> typename X<T, int>::t;\n\
         long f(...);\n\
         long p = sizeof(f(1));\n\
         template <> struct X<int, int> { using t = int; };\n\
         static_assert(sizeof(f(1)) == sizeof(int));\n",
    );
    let xs = specs(&ast, DeclKind::CXXRecord, "X");
    assert_eq!(xs.len(), 1);
    let x = xs[0];
    assert!(!invalid(&ast, x), "explicit specialization X<int, int> must be valid");
    assert_eq!(ast.decls.subst_failed(x), None, "stale kind-4 mark must be cleared");
    assert!(ast.definition(x).is_some(), "X<int, int> must be defined");
    // Only the live valid specialization of `f` is reachable from `specs_of` / `find_spec`.
    let ft = named(&ast, DeclKind::FunctionTemplate, "f")[0];
    let live = ast.templates.specs_of(ft);
    assert_eq!(live.len(), 1, "retired failed f<int> must not remain in specs_of");
    let live_spec = live[0];
    assert!(!invalid(&ast, live_spec), "re-deduced f<int> must be valid");
    assert_eq!(ast.decls.subst_failed(live_spec), None);
    let canonical = ast.types.canonical_args(ast.spec_info(live_spec).unwrap().1.args);
    assert_eq!(ast.templates.find_spec(ft, canonical), Some(live_spec));
    // The retired failed `f<int>` stays in the arena with `FunctionSignature` (verified by `clean`).
    let all_f = specs(&ast, DeclKind::Function, "f");
    assert_eq!(all_f.len(), 2);
    assert_eq!(marks(&ast), [(DeclKind::Function, SubstFailedKind::FunctionSignature)]);
}

/// The kinds' values are fixed (spec/ast.md §4.1) and round-trip.
#[test]
fn kind_values_are_fixed() {
    for (i, k) in SubstFailedKind::ALL.into_iter().enumerate() {
        assert_eq!(usize::from(k.value()), i + 1);
        assert_eq!(SubstFailedKind::from_value(k.value()), Some(k));
    }
    assert_eq!(SubstFailedKind::from_value(0), None);
    assert_eq!(SubstFailedKind::from_value(7), None);
}
