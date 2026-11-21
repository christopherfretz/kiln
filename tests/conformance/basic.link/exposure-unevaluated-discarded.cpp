// [basic.link]/14: a declaration names an entity whatever the context of
// the expression or type naming it; nothing in /15 ignores an unevaluated
// operand, a discarded statement or a declarator. In a module interface,
// naming a TU-local entity there is an exposure (/17): in an inline
// function's body a `sizeof`, `decltype`, `noexcept` or requires-expression
// operand, a type in `sizeof`, an `if constexpr` branch discarded (also in
// a template definition, and so the specialization using it, /15.5); in a
// declaration, a `decltype` or `noexcept` operand of its type, a `sizeof`
// template argument and a member's `decltype` type. GCC 16.1 rejects only
// the two template definitions (it folds the rest away before its check);
// clang 22 warns for the other body uses except `sizeof(S)` and accepts
// the declarators silently (a deviation, docs/status/deviations.md). An
// enumerator and a specialization that is not TU-local get clang 22's
// warning instead (exposure-unevaluated-warning.cpp).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 41:19 error "declaration of 'size' exposes TU-local entity 'helper'"
//@ diag: 42:19 error "declaration of 'type' exposes TU-local entity 'hidden'"
//@ diag: 43:20 error "declaration of 'no_throw' exposes TU-local entity 'helper'"
//@ diag: 44:20 error "declaration of 'required' exposes TU-local entity 'helper'"
//@ diag: 45:19 error "declaration of 'type_size' exposes TU-local entity 'S'"
//@ diag: 46:19 error "declaration of 'discarded' exposes TU-local entity 'helper'"
//@ diag: 47:38 error "declaration of 't_discarded' exposes TU-local entity 'helper'"
//@ diag: 48:19 error "declaration of 'use_discarded' exposes TU-local entity 't_discarded<int>'"
//@ diag: 49:38 error "declaration of 't_size' exposes TU-local entity 'helper'"
//@ diag: 50:19 error "declaration of 'use_size' exposes TU-local entity 't_size<int>'"
//@ diag: 52:19 error "declaration of 'fp' exposes TU-local entity 'helper'"
//@ diag: 53:25 error "declaration of 'v1' exposes TU-local entity 'hidden'"
//@ diag: 54:13 error "declaration of 'g1' exposes TU-local entity 'helper'"
//@ diag: 55:13 error "declaration of 'g2' exposes TU-local entity 'hidden'"
//@ diag: 56:13 error "declaration of 'g3' exposes TU-local entity 'helper'"
//@ diag: 58:26 error "declaration of 'v2' exposes TU-local entity 'hidden'"
//@ diag: 59:15 error "declaration of 'C' exposes TU-local entity 'helper'"
export module M;
static int helper() { return 1; }
static int hidden;
namespace { struct S { int v; }; }

export inline int size() { return sizeof(helper()); }
export inline int type() { decltype(hidden) x = 0; return x; }
export inline bool no_throw() { return noexcept(helper()); }
export inline bool required() { return requires { helper(); }; }
export inline int type_size() { return sizeof(S); }
export inline int discarded() { if constexpr (false) { return helper(); } return 0; }
export template <class T> inline int t_discarded(T) { if constexpr (sizeof(T) == 1) { return helper(); } return 0; }
export inline int use_discarded() { return t_discarded(1); }
export template <class T> inline int t_size(T) { return sizeof(helper()); }
export inline int use_size() { return t_size(1); }

decltype(helper) *fp;
export decltype(hidden) v1;
export auto g1() -> decltype(helper());
export void g2(decltype(hidden));
export void g3() noexcept(noexcept(helper()));
template <int N> struct A {};
export A<sizeof(hidden)> v2;
export struct C { decltype(helper()) m; };
