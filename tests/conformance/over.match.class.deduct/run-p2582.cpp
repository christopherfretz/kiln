//@ expect: run
//@ std: c++23
//@ skip(clang): neither clang 19 nor clang 22 implements P2582; the output is checked by hand
//@ stdout: 7 2.5
//@ stdout: x 3
// [over.match.class.deduct]/1 (C++23, P2582R1) at run time: the type deduced
// from an inherited constructor's guide is constructed through that inherited
// constructor ([class.inhctor.init]).
extern "C" int printf(const char *, ...);
template <class T> struct Base {
  T v;
  Base(T t) : v(t) {}
};
template <class T> struct Derived : Base<T> {
  using Base<T>::Base;
  T twice() const { return this->v + this->v; }
};
template <class T, class U> struct Pair {
  T a;
  U b;
  Pair(T x, U y) : a(x), b(y) {}
};
template <class U, class T> struct Swapped : Pair<T, U> {
  using Pair<T, U>::Pair;
};
int main() {
  Derived d(7);
  Derived e(1.25);
  printf("%d %g\n", d.v, e.twice());
  Swapped s('x', 3);
  static_assert(__is_same(decltype(s), Swapped<int, char>));
  printf("%c %d\n", s.a, s.b);
}
