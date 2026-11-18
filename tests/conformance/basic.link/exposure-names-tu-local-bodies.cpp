// [basic.link]/14-17: in a module interface, the body of an inline or
// constexpr function (an inline function template, an out-of-class inline
// member definition) is not ignored by /15, so naming a TU-local entity in
// it is an exposure: a call, an address, a local variable's type, a call in
// a lambda (reported at the lambda), a call to a TU-local inline function,
// a call on a branch that is never taken. Uses that fold away (a discarded
// expression naming a function, sizeof) are not; see
// exposure-names-tu-local-accepted.cpp. GCC 16.1 rejects each (it reports
// X::f at the qualified name, kiln at f); clang 22 only warns.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 34:19 error "declaration of 'body_type' exposes TU-local entity 'S'"
//@ diag: 35:22 error "declaration of 'cx' exposes TU-local entity 'helper'"
//@ diag: 36:36 error "declaration of 'templ' exposes TU-local entity 'helper'"
//@ diag: 37:41 error "declaration of 'via_lambda' exposes TU-local entity 'helper'"
//@ diag: 38:19 error "declaration of 'via_tu_local' exposes TU-local entity 'local_inline'"
//@ diag: 39:20 error "declaration of 'address' exposes TU-local entity 'target'"
//@ diag: 40:19 error "declaration of 'calls' exposes TU-local entity 'helper'"
//@ diag: 41:19 error "declaration of 'stored' exposes TU-local entity 'helper'"
//@ diag: 42:19 error "declaration of 'unfolded' exposes TU-local entity 'helper'"
//@ diag: 43:19 error "declaration of 'spec_local' exposes TU-local entity 'S'"
//@ diag: 45:15 error "declaration of 'f' exposes TU-local entity 'helper'"
export module M;
static int helper() { return 1; }
static int target;
namespace {
struct S { int v; };
inline int local_inline() { return helper(); }
}
template <class T> struct W { T t; };

export inline int body_type() { S s{1}; return s.v + helper(); }
export constexpr int cx(int x) { return x ? 0 : helper(); }
export template <class T> inline T templ(T t) { return helper() + t; }
export inline int via_lambda() { return [] { return helper(); }(); }
export inline int via_tu_local() { return local_inline(); }
export inline int *address() { return &target; }
export inline int calls() { (void)helper(); return 0; }
export inline int stored() { auto p = &helper; return p(); }
export inline int unfolded() { if (false) return helper(); return 0; }
export inline int spec_local() { W<S> *p = nullptr; return p ? 1 : 0; }
export struct X { int f(); };
inline int X::f() { return helper(); }
