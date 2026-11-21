// [basic.link]/16.5: a specialization of a template whose instantiated
// declaration is an exposure is TU-local. The instantiation of an inline
// function template (or of a constexpr member of a class template) whose
// body names a TU-local function, found by argument-dependent lookup in the
// instantiation or named in the template, is TU-local, also through another
// such instantiation; so in a module interface an inline function that
// calls one, or takes its address, is an exposure, reported at it once the
// unit's instantiations are done. A template that names a TU-local entity
// itself is one too (`named`). GCC 16.1 rejects each, reporting the
// specialization instead; clang 22 accepts the ones found by
// argument-dependent lookup and only warns for the others (a deviation,
// docs/status/deviations.md).
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 accepts or only warns
//@ strict-diags
//@ diag: 37:31 error "declaration of 'named' exposes TU-local entity 'helper'"
//@ diag: 39:19 error "declaration of 'call' exposes TU-local entity 'by_adl<N::A>'"
//@ diag: 40:19 error "declaration of 'call_constexpr' exposes TU-local entity 'cx_by_adl<N::A>'"
//@ diag: 41:19 error "declaration of 'call_indirect' exposes TU-local entity 'indirect<N::A>'"
//@ diag: 42:19 error "declaration of 'call_member' exposes TU-local entity 'g'"
//@ diag: 43:19 error "declaration of 'call_named' exposes TU-local entity 'named<int>'"
//@ diag: 44:12 error "declaration of 'module_linkage' exposes TU-local entity 'by_adl<N::A>'"
//@ diag: 45:38 error "declaration of 'in_template' exposes TU-local entity 'by_adl<N::A>'"
export module M;
static int helper() { return 1; }
namespace N {
struct A {};
static int adl(A) { return 1; }
}
template <class T> inline int by_adl(T x) { return adl(x); }
template <class T> constexpr int cx_by_adl(T x) { return adl(x); }
template <class T> inline int indirect(T x) { return by_adl(x); }
template <class T> struct W {
  constexpr int g() { return adl(T{}); }
};
template <class T> inline int named(T) { return helper(); }

export inline int call() { return by_adl(N::A{}); }
export inline int call_constexpr() { return cx_by_adl(N::A{}); }
export inline int call_indirect() { return indirect(N::A{}); }
export inline int call_member() { return W<N::A>{}.g(); }
export inline int call_named() { return named(0); }
inline int module_linkage() { auto f = &by_adl<N::A>; return f({}); }
export template <class T> inline int in_template() { return by_adl(N::A{}); }
