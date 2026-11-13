// [temp.spec.partial.match]/2, [temp.deduct.type]/20, [temp.arg.nontype]/1:
// a partial specialization matches only when each deduced value fits its
// parameter's type: a fixed type takes only a value of that type (an
// `int` parameter no `bool`, `char` or enumerator: the type of the
// parameter differs from the type the `auto` parameter takes), and a
// placeholder type (`auto *P`, `auto Q::*M`) only a value whose type it
// deduces from. GCC 16.1 agrees; clang 19/22 convert integral values
// (WP-1405). The qualification case `C<(const int *)&x>` is
// nttp-pointer-qualification-differs.cpp (GCC matches it). Before WP-1105 kiln let `nullptr`,
// `&x`, `1.0` or a scoped enumerator of an `auto` primary bind `int N`,
// and `0` bind `auto *P`; deduced packs are checked element by element.
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 match a partial specialization whose non-type parameter type differs from the primary's
//@ expect: run
//@ stdout: A 100000000 B 1000 C 10 D 10 F 100 G 100 H 100 K 10 M 01 N 10 P 10 R 001
extern "C" int printf(const char *, ...);
enum E { e1 = 1 };
enum E2 { f1 = 1 };
enum class SE { s1 = 1 };
struct Q { int m; };
struct S { int a; constexpr S(int x) : a(x) {} };
struct T { int a; constexpr T(int x) : a(x) {} constexpr T(S s) : a(s.a) {} };
int x;

template <auto V> struct A { static constexpr int v = 0; };
template <int N> struct A<N> { static constexpr int v = 1; };
template <auto V> struct B { static constexpr int v = 0; };
template <auto *P> struct B<P> { static constexpr int v = 1; };
template <auto V> struct C { static constexpr int v = 0; };
template <int *P> struct C<P> { static constexpr int v = 1; };
template <auto V> struct D { static constexpr int v = 0; };
template <E V> struct D<V> { static constexpr int v = 1; };
template <auto V> struct F { static constexpr int v = 0; };
template <bool V> struct F<V> { static constexpr int v = 1; };
template <auto V> struct G { static constexpr int v = 0; };
template <auto Q::*M> struct G<M> { static constexpr int v = 1; };
template <auto V> struct H { static constexpr int v = 0; };
template <double D> struct H<D> { static constexpr int v = 1; };
template <auto V> struct K { static constexpr int v = 0; };
template <T t> struct K<t> { static constexpr int v = 1; };
template <auto V> struct M { static constexpr int v = 0; };
template <E V> struct M<V> { static constexpr int v = 1; };
template <auto V> struct N { static constexpr int v = 0; };
template <decltype(nullptr) V> struct N<V> { static constexpr int v = 1; };
template <class U, U... Vs> struct P { static constexpr int v = 0; };
template <int... Is> struct P<int, Is...> { static constexpr int v = 1; };
template <int H, long... Vs> struct R { static constexpr int v = 0; };
template <char... Cs> struct R<1, Cs...> { static constexpr int v = 1; };

int main() {
  printf("A %d%d%d%d%d%d%d%d%d ", A<1>::v, A<true>::v, A<'c'>::v, A<e1>::v, A<SE::s1>::v,
         A<nullptr>::v, A<&x>::v, A<1.0>::v, A<300000000000L>::v);
  printf("B %d%d%d%d ", B<&x>::v, B<0>::v, B<nullptr>::v, B<&Q::m>::v);
  printf("C %d%d ", C<&x>::v, C<nullptr>::v);
  printf("D %d%d ", D<e1>::v, D<1>::v);
  printf("F %d%d%d ", F<true>::v, F<2>::v, F<1>::v);
  printf("G %d%d%d ", G<&Q::m>::v, G<0>::v, G<&x>::v);
  printf("H %d%d%d ", H<1.0>::v, H<1.0f>::v, H<1>::v);
  printf("K %d%d ", K<T{1}>::v, K<S{1}>::v);
  printf("M %d%d ", M<f1>::v, M<e1>::v);
  printf("N %d%d ", N<nullptr>::v, N<0>::v);
  printf("P %d%d ", P<int, 1, 2>::v, P<long, 1>::v);
  printf("R %d%d%d\n", R<1, 2, 3>::v, R<1, 300>::v, R<1>::v);
}
