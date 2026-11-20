// [over.match.class.deduct]/3, [temp.dep.type]/6-7: an alias template's
// guides formed from constructors whose parameter types name a static
// member function (`A1<sf()>`), a variable template (`A1<vt<char>>`), a
// member class template (`Tm<V>`), a member alias (`Al`) or the
// injected-class-name's enumerator (`A1<B::X + 7>`) of the current
// instantiation: each is named in the specialization the alias's
// arguments make. clang 22 drops these guides; GCC 16.1 forms them.
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 22 forms no deduction guide from these constructors ("no viable constructor or deduction guide")
//@ stdout: sf|vt|Tm|Al|B::X|
#include <cstdio>
template <int> struct A1 {};
template <class> struct A3 {};
template <typename V, int N> struct B {
  enum E { X };
  static constexpr int sf() { return N; }
  template <class Q> static constexpr int vt = sizeof(Q) + N;
  template <class Q> struct Tm {};
  using Al = A3<V>;
  B(A1<sf()>, V) { std::printf("sf|"); }
  B(A1<vt<char>>, V, int) { std::printf("vt|"); }
  B(Tm<V>, V) { std::printf("Tm|"); }
  B(Al, V) { std::printf("Al|"); }
  B(A1<B::X + 7>, V, short) { std::printf("B::X|"); }
};
template <class T> using U = B<T, 5>;
int main() {
  U u1{A1<5>(), 1};
  U u2{A1<6>(), 1, 0};
  U u3{B<int, 5>::Tm<int>(), 1};
  U u4{A3<int>(), 1};
  U u5{A1<7>(), 1, (short)0};
  std::printf("\n");
}
