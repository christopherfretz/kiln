// [temp.deduct.guide]/3, [temp.inst]/3, [over.match.class.deduct]/1: a
// deduction guide declared for a member class template of a class
// template (a non-template guide naming the enclosing parameter, one for
// a fixed type, a guide template) is instantiated with the enclosing
// specialization, its types substituted; class template argument
// deduction for `A<int>::B` uses the instantiated guides. Also in a
// partial specialization (`C<R(Args...)>`). Reduced from g++.dg
// cpp1z/class-deduction98.C and class-deduction106.C.
//@ expect: run
//@ std: c++20
//@ stdout: 8 8 8 1
#include <cstdio>
template <class T> struct A {
  template <class U> struct B {
    int n;
    template <class V> B(V) : n(sizeof(V)) {}
  };
  B(T) -> B<T>;
  B(const char *) -> B<long>;
  template <class V> B(V *) -> B<T *>;
};
template <class> struct C;
template <class R, class... Args> struct C<R(Args...)> {
  template <class T> struct D { D(T) {} };
  D(decltype(nullptr)) -> D<void *>;
};
int main() {
  A<int>::B b1(2.5);
  A<int>::B b2("x");
  double d = 1;
  A<short>::B b3(&d);
  static_assert(__is_same(decltype(b1), A<int>::B<int>));
  static_assert(__is_same(decltype(b2), A<int>::B<long>));
  static_assert(__is_same(decltype(b3), A<short>::B<short *>));
  static_assert(__is_same(decltype(C<char(int)>::D{nullptr}), C<char(int)>::D<void *>));
  static_assert(__is_same(decltype(C<char(int)>::D{0}), C<char(int)>::D<int>));
  std::printf("%d %d %d %d\n", b1.n, b2.n, b3.n, int(sizeof(C<void()>::D{nullptr}) >= 1));
}
