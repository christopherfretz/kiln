// [dcl.fct.def.general]/8 (`__PRETTY_FUNCTION__`, clang's
// PredefinedExpr::ComputeName): a deduced or explicit template argument
// of function, function pointer or member pointer type prints in the
// ` [T = ...]` list as clang prints the type, a non-throwing one with a
// trailing `noexcept` and a potentially-throwing one with nothing (not
// `noexcept(true)` / `noexcept(false)`), including a specialization's
// instantiated `noexcept(B)`; and a member of a nested class template or
// a partial specialization in a namespace prints its scope with every
// argument scoped. Matches clang 19 and 22 byte for byte (WP-753).
//@ expect: run
//@ std: c++17
//@ stdout: static void N::A<N::X>::B<N::M::Y>::f() [T = N::X, U = N::M::Y]
//@ stdout: static void N::A<N::X>::B<N::M::Y>::g(V) [T = N::X, U = N::M::Y, V = N::X]
//@ stdout: static void N::P<N::X, N::M::Y>::f() [T = N::X, U = N::M::Y]
//@ stdout: static void N::P<N::X, N::M::Y>::g(V) [T = N::X, U = N::M::Y, V = N::M::Y]
//@ stdout: static void N::P<N::X, N::M::Y>::In::f() [T = N::X, U = N::M::Y]
//@ stdout: static void N::P<N::X *, N::X>::Q<N::M::Y>::f() [T = N::X *, U = N::X, U = N::M::Y]
//@ stdout: void t(T) [T = void (*)()]
//@ stdout: void t(T) [T = void (*)() noexcept]
//@ stdout: void t(T) [T = void (S::*)() noexcept]
//@ stdout: void t(T) [T = void (S::*)() const]
//@ stdout: void t(T) [T = int S::*]
//@ stdout: void u() [T = void () noexcept]
//@ stdout: void u() [T = void ()]
//@ stdout: void u() [T = void (S::*)() const noexcept]
//@ stdout: void u() [T = void (*)(int) noexcept]
//@ stdout: void u() [T = void (*(*)(int) noexcept)() noexcept]
#include <cstdio>
namespace N {
struct X {};
namespace M { struct Y {}; }
template <class T> struct A {
  template <class U> struct B {
    static void f() { puts(__PRETTY_FUNCTION__); }
    template <class V> static void g(V) { puts(__PRETTY_FUNCTION__); }
  };
};
template <class T, class U> struct P { static void f() { puts(__PRETTY_FUNCTION__); } };
template <class T> struct P<T, M::Y> {
  static void f() { puts(__PRETTY_FUNCTION__); }
  template <class V> static void g(V) { puts(__PRETTY_FUNCTION__); }
  struct In { static void f() { puts(__PRETTY_FUNCTION__); } };
};
template <class T> struct P<T*, X> {
  template <class U> struct Q { static void f() { puts(__PRETTY_FUNCTION__); } };
};
}
void g0() {}
void g1() noexcept {}
struct S { void m() noexcept {} void n() const {} int d; };
template <class T> void t(T) { puts(__PRETTY_FUNCTION__); }
template <class T> void u() { puts(__PRETTY_FUNCTION__); }
int main() {
  N::A<N::X>::B<N::M::Y>::f();
  N::A<N::X>::B<N::M::Y>::g(N::X{});
  N::P<N::X, N::M::Y>::f();
  N::P<N::X, N::M::Y>::g(N::M::Y{});
  N::P<N::X, N::M::Y>::In::f();
  N::P<N::X*, N::X>::Q<N::M::Y>::f();
  t(&g0); t(&g1); t(&S::m); t(&S::n); t(&S::d);
  u<void() noexcept>(); u<void()>(); u<void (S::*)() const noexcept>(); u<void(*)(int) noexcept>();
  u<void (*(*)(int) noexcept)() noexcept>();
}
