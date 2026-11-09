// [temp.spec.partial.order], [temp.deduct.type]/9, CWG1432: of two partial
// specializations that differ only in a trailing pack (`C<T, U...>` and
// `C<T>`), the one without the pack is more specialized; the pack one still
// takes the longer argument lists. kiln's list deduction fails a pack
// expansion against a non-pack parameter, and clang repairs the same order
// with its speculative CWG1432 tie-break (`getMoreSpecialized`), which
// kiln mirrors (WP-1105); for class and variable templates, constrained
// specializations, packs nested in a template argument and function types.
//@ std: c++20
//@ expect: run
//@ stdout: C 210 D 231 CT 21 v 210 S 21 P 210 Q 2 W 21
extern "C" int printf(const char *, ...);
template <class... T> struct L {};
template <class... T> struct C { static constexpr int v = 0; };
template <class T, class... U> struct C<T, U...> { static constexpr int v = 1; };
template <class T> struct C<T> { static constexpr int v = 2; };
template <class X> struct D { static constexpr int v = 0; };
template <class T, class... U> struct D<L<T, U...>> { static constexpr int v = 1; };
template <class T> struct D<L<T>> { static constexpr int v = 2; };
template <class T, class U> struct D<L<T, U>> { static constexpr int v = 3; };
template <class... T> struct CT;
template <class T, class U> struct CT<T, U> { static constexpr int v = 2; };
template <class T, class U, class... V> struct CT<T, U, V...> { static constexpr int v = 1; };
template <class... T> constexpr int v = 0;
template <class T, class... U> constexpr int v<T, U...> = 1;
template <class T> constexpr int v<T> = 2;
template <int... N> struct S { static constexpr int v = 0; };
template <int N, int... M> struct S<N, M...> { static constexpr int v = 1; };
template <int N> struct S<N> { static constexpr int v = 2; };
template <class A, class... B> struct P { static constexpr int v = 0; };
template <class A, class B, class... R> struct P<A, B, R...> { static constexpr int v = 1; };
template <class A, class B> struct P<A, B> { static constexpr int v = 2; };
template <class T> concept Any = true;
template <class... T> struct Q { static constexpr int v = 0; };
template <Any T, class... U> struct Q<T, U...> { static constexpr int v = 1; };
template <class T> struct Q<T> { static constexpr int v = 2; };
template <class X> struct W { static constexpr int v = 0; };
template <class T, class... U> struct W<void(T, U...)> { static constexpr int v = 1; };
template <class T> struct W<void(T)> { static constexpr int v = 2; };

int main() {
  printf("C %d%d%d ", C<int>::v, C<int, int>::v, C<>::v);
  printf("D %d%d%d ", D<L<int>>::v, D<L<int, int>>::v, D<L<int, int, int>>::v);
  printf("CT %d%d ", CT<int, double>::v, CT<int, double, char>::v);
  printf("v %d%d%d ", v<int>, v<int, int>, v<>);
  printf("S %d%d ", S<1>::v, S<1, 2>::v);
  printf("P %d%d%d ", P<int, int>::v, P<int, int, int>::v, P<int>::v);
  printf("Q %d ", Q<int>::v);
  printf("W %d%d\n", W<void(int)>::v, W<void(int, int)>::v);
}
