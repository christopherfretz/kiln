// [dcl.fct.def.general]/8 (`__PRETTY_FUNCTION__`, clang 22's
// PredefinedExpr::ComputeName): members of a partial specialization in a
// namespace, of a member class template of one, of its local class and
// of lambdas in it (a generic lambda's invented parameter prints `auto`
// in the signature, `v:auto` in the list, also when the lambda is
// instantiated with its enclosing template) print every scope's argument
// scoped (`N::P<N::X *>`). clang 19 and `-fclang-compat=19` name a
// closure type `(anonymous class)`, so the test requires 22 (WP-753).
//@ expect: run
//@ std: c++20
//@ requires: clang>=22
//@ stdout: static void N::P<N::X *>::f()::L::g() [T = N::X *, U = N::M::Y]
//@ stdout: auto N::P<N::X *>::f()::(lambda)::operator()() const [T = N::X *, U = N::M::Y]
//@ stdout: auto N::P<N::X *>::f()::(lambda)::operator()(auto) const [T = N::X *, U = N::M::Y, v:auto = N::X]
//@ stdout: static void N::P<N::X *>::Q<N::M::Y>::h() [T = N::X *, U = N::M::Y, V = N::M::Y]
//@ stdout: auto N::P<N::X *>::Q<N::M::Y>::h()::(lambda)::operator()() const [T = N::X *, U = N::M::Y, V = N::M::Y]
//@ stdout: auto N::P<N::X *>::(lambda)::operator()() const [T = N::X *, U = N::M::Y]
//@ stdout: static void N::P<N::M::Y, N::X>::Q<N::X>::h() [T = N::M::Y, U = N::X, V = N::X]
//@ stdout: static void N::R<N::X, N::X>::S<N::M::Y>::h() [T = N::X, U = N::X, V = N::M::Y]
//@ stdout: static void N::R<N::X, int>::S<N::M::Y>::h() [T = N::X, U = int, V = N::M::Y]
#include <cstdio>
namespace N {
struct X {};
namespace M { struct Y {}; }
template <class T, class U = M::Y> struct P {};
template <class T> struct P<T*, M::Y> {
  static void f() {
    struct L { static void g() { puts(__PRETTY_FUNCTION__); } };
    L::g();
    auto l = [] { puts(__PRETTY_FUNCTION__); };
    l();
    auto gl = [](auto v) { puts(__PRETTY_FUNCTION__); };
    gl(X{});
  }
  template <class V> struct Q {
    static void h() { puts(__PRETTY_FUNCTION__); auto l = [] { puts(__PRETTY_FUNCTION__); }; l(); }
  };
  std::nullptr_t dm = ([] { puts(__PRETTY_FUNCTION__); }(), nullptr);
};
template <class T> struct P<T, X> {
  template <class V> struct Q { static void h() { puts(__PRETTY_FUNCTION__); } };
};
template <class T, class U> struct R { template <class V> struct S { static void h() { puts(__PRETTY_FUNCTION__); } }; };
template <class T> struct R<T, T> { template <class V> struct S { static void h() { puts(__PRETTY_FUNCTION__); } }; };
}
int main() {
  N::P<N::X*>::f();
  N::P<N::X*>::Q<N::M::Y>::h();
  N::P<N::X*> p;
  N::P<N::M::Y, N::X>::Q<N::X>::h();
  N::R<N::X, N::X>::S<N::M::Y>::h();
  N::R<N::X, int>::S<N::M::Y>::h();
}
