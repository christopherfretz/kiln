// [basic.link]/14.1, /15.5 (WP-1846): the body of a function template
// that is not inline is ignored, but not the deduced return type of its
// specializations. A specialization whose deduced return type names a
// local class or closure type (through a template argument too) whose
// declarations name a TU-local entity (a data member's type, a base, the
// body of a constexpr member function, a returned lambda's body; also one
// found by argument-dependent lookup in the instantiation, or through an
// instantiation that is TU-local so) is an exposure, so it is TU-local
// (/15.5), as WP-1820 decides for a function that is not a template; a
// declaration naming it is an exposure where a use of any TU-local
// specialization is (the error at that declaration, naming the
// specialization). A local class the return type does not name, a member
// function that is not inline, a specialization named only in an ignored
// body and a template never instantiated expose nothing. GCC 16.1 rejects
// the instantiated member function or lambda itself, where its body names
// the entity (`lambda`, `adl_member`, `adl_lambda`; not the others), and
// `not_returned`'s; clang 22 warns at some of the uses.
//@ expect: compile-fail
//@ std: c++20
//@ skip(clang): clang 22 only warns
//@ strict-diags
//@ diag: 49:19 error "declaration of 'u_field' exposes TU-local entity 'field<int>'"
//@ diag: 50:19 error "declaration of 'u_base' exposes TU-local entity 'base<int>'"
//@ diag: 51:19 error "declaration of 'u_lambda' exposes TU-local entity 'lambda<int>'"
//@ diag: 52:19 error "declaration of 'u_adl' exposes TU-local entity 'adl_member<N::A>'"
//@ diag: 53:19 error "declaration of 'u_boxed' exposes TU-local entity 'boxed<int>'"
//@ diag: 54:19 error "declaration of 'u_adl_lambda' exposes TU-local entity 'adl_lambda<N::A>'"
//@ diag: 55:19 error "declaration of 'u_member_of' exposes TU-local entity 'f'"
//@ diag: 56:19 error "declaration of 'u_chain' exposes TU-local entity 'chain<N::A>'"
export module M;
static int helper() { return 1; }
namespace { struct S { int v; }; }
namespace N { struct A {}; static int adl(A) { return 2; } }
template <class T> struct Box { T t; };
template <class T> inline int calls(T t) { return adl(t); }

template <class T> auto field(T) { struct L { S s; }; return L(); }
template <class T> auto base(T) { struct L : S {}; return L(); }
template <class T> auto lambda(T) { return [] { return helper(); }; }
template <class T> auto adl_member(T) { struct L { constexpr int g() { return adl(T()); } }; return L(); }
template <class T> auto boxed(T) { struct L { constexpr int g() { return helper(); } }; return Box<L>(); }
template <class T> auto adl_lambda(T) { return [](T t) { return adl(t); }; }
template <class T> struct Cls { auto f() { struct L { constexpr int g() { return helper(); } }; return L(); } };
template <class T> auto chain(T) { struct L { constexpr int g() { return calls(T()); } }; return L(); }
template <class T> auto not_returned(T) { struct L { constexpr int g() { return helper(); } }; L().g(); return 0; }
template <class T> auto not_inline(T) { struct L { int g() { return helper(); } }; return L(); }
template <class T> auto never(T) { struct L { constexpr int g() { return helper(); } }; return L(); }

export inline int u_field() { return sizeof(field(1)); }
export inline int u_base() { return sizeof(base(1)); }
export inline int u_lambda() { return lambda(1)(); }
export inline int u_adl() { return adl_member(N::A()).g(); }
export inline int u_boxed() { return sizeof(boxed(1)); }
export inline int u_adl_lambda() { return adl_lambda(N::A())(N::A()); }
export inline int u_member_of() { return sizeof(Cls<int>().f()); }
export inline int u_chain() { return sizeof(chain(N::A())); }
export inline int u_not_returned() { return not_returned(1); }
export inline int u_not_inline() { return not_inline(1).g(); }
export int u_ignored() { return lambda(2)() + field(2).s.v; }
