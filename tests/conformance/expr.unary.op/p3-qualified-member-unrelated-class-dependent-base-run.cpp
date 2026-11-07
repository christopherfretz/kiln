// [expr.unary.op]/3, [class.mfct.non.static]/2, [temp.dep.type]: inside a
// member of a class template with a dependent base, `Int::f` naming a
// non-static member of a class `Int` that `*this` might derive from is
// kept as the implicit `this->Int::f` until instantiation, but as the
// operand of `&` it is the qualified name itself and forms a pointer to
// member: clang builds no implicit member access for an address-of
// operand. Unrelated at instantiation, `&Int::f` must still be valid.
// abseil absl/functional/any_invocable_test.cc (`&Int::MemberFunctionAdd`
// in a gtest typed test, WP-686). An overload set (`&Int::f` naming two
// members) is p3-qualified-overload-set-dependent-base-run.cpp (WP-715).
// Verified against clang 19.
//@ expect: run
//@ stdout: 6 8 12
#include <cstdio>
namespace {
struct Int {
  int v;
  int Add(int const &b, int c) noexcept { return v + b + c; }
  int Twice(int b) { return 2 * b; }
  int m;
};
}
template <class P> struct Base {};
template <class P> struct D : Base<P> {
  int add();
  int twice() {
    int (Int::*f)(int) = &Int::Twice;
    int Int::*pm = &Int::m;
    Int i{1, 4};
    return (i.*f)(2) + i.*pm;
  }
};
template <class P> int D<P>::add() {
  using F = int (Int::*)(int const &, int) noexcept;
  F f = &Int::Add;
  Int i{1, 0};
  return (i.*f)(2, 3);
}
template <class P> struct E : Base<P> {
  int g() { auto f = &Int::Add; Int i{2, 0}; return (i.*f)(4, 6); }
};
int main() { std::printf("%d %d %d\n", D<int>().add(), D<char>().twice(), E<int>().g()); }
