// [over.match.class.deduct]/3, [temp.dep.type]/6-7: the guides of an
// alias template (and the inherited-constructor guides of a derived class
// template, /1.4) are formed from constructors whose parameter types name
// members of the class template's current instantiation (`A1<X>` with an
// enumerator, `A1<In::k>`, `A1<In::Z>`, `A3<E>`); substituted with the
// alias's arguments they become members of the specialization named
// there, which may be a member class template of another one (`O<int>::I`).
// clang 22 drops these guides; GCC 16.1 forms them.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 forms no deduction guide from these constructors ("no viable constructor or deduction guide")
//@ stdout: X 3|k 3|Z 3|X 5|k 5|Z 5|I|I|J|E 9|
#include <cstdio>
template <int> struct A1 {};
template <class> struct A3 {};
template <typename V, int N> struct B {
  enum E { X = N };
  struct In { static constexpr int k = N + 1; enum F { Z = N + 2 }; };
  B(A1<X>, V) { std::printf("X %d|", N); }
  B(A1<In::k>, V, int) { std::printf("k %d|", N); }
  B(A1<In::Z>, V, long) { std::printf("Z %d|", N); }
};
template <class T> struct D : B<T, 3> { using B<T, 3>::B; };
template <class T> using U = B<T, 5>;
template <class T> struct O {
  template <class V, int N> struct I {
    enum E { X = N };
    I(A1<N>, V) { std::printf("I|"); }
    I(A3<E>, V, int) { std::printf("E %d|", N + 2); }
  };
};
template <class V> using OI = O<int>::I<V, 7>;
template <class T> struct P { template <class V> struct J { J(V) { std::printf("J|"); } }; };
template <class V> using PJ = P<int>::J<V>;
int main() {
  D d1{A1<3>(), 1};
  D d2{A1<4>(), 1, 0};
  D d3{A1<5>(), 1, 0L};
  U u1{A1<5>(), 1};
  U u2{A1<6>(), 1, 0};
  U u3{A1<7>(), 1, 0L};
  O<int>::I o1{A1<7>(), 1};
  OI o2{A1<7>(), 1};
  PJ p{1};
  OI o3{A3<O<int>::I<int, 7>::E>(), 1, 0};
  std::printf("\n");
}
