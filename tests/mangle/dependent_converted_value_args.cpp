// Mangling differential (Itanium C++ ABI 5.1.5.8, [temp.type]/1; WP-3321):
// in a function template's signature, a non-dependent non-type argument
// of a dependent class template-id mangles as its converted value, as
// clang's converted argument list does: `It<T, 1 == 1>` is `2ItIT_Lb1EE`,
// `E(1)` is `L1E1E`, `(int *)0` is `LPi0E`, `'a'` for a `long` parameter
// is `Ll97E`, `P{0 + 1}` is `Xtl1PLi1EEE`. An argument for a parameter
// whose type is the dependent `T` stays as written (`Li1E`, `Lj1E`), and a
// value-dependent one stays an expression (`XT_E`).
namespace cv {
enum E { e0, e1 };
enum class F : int { a, b };
int x;
struct P {
  int a;
};
template <class T, bool B> struct It {};
template <class T, E e> struct A {};
template <class T, F f> struct B {};
template <class T, int *p> struct C {};
template <class T, int &r> struct D {};
template <class T, P v> struct G {};
template <class U, U v> struct W {};
template <class T, long n = 1> struct L {};
template <class T, int M, unsigned N> struct Z {};
constexpr int k = 3;
template <int, int...> struct V {};
template <int, int, int...> struct V2 {};
template <int, int> struct P2 {};
template <class T, int... Ns> struct Wp {};
template <class T, auto... Ns> struct WA {};
template <class U, U... vs> struct WD {};
template <class T, int = 1, int...> struct D2 {};

template <class T> void i1(It<T, 1 == 1>) {}
template <class T> void i2(It<T, true>) {}
template <class T> void i3(It<It<T, !false>, 1>) {}
template <class T> void a1(A<T, e1>) {}
template <class T> void a2(A<T, E(1)>) {}
template <class T> void b1(B<T, F::b>) {}
template <class T> void b2(B<T, F(1)>) {}
template <class T> void c1(C<T, nullptr>) {}
template <class T> void c2(C<T, (int *)0>) {}
template <class T> void c3(C<T, &x>) {}
template <class T> void c4(C<T, (&x)>) {}
template <class T> void d1(D<T, x>) {}
template <class T> void d2(D<T, (x)>) {}
template <class T> void g1(G<T, P{1}>) {}
template <class T> void g2(G<T, P{0 + 1}>) {}
template <class T> void w1(W<T, 1>) {}
template <class T> void w2(W<T, 1u>) {}
template <class T> void l1(L<T, 2>) {}
template <class T> void l2(L<T, 'a'>) {}
template <class T> void l3(L<T>) {}
template <int M, class T> void z1(Z<T, M, 1 + 2>) {}
template <int... Ns> void pk1(V<k, Ns...>) {}
template <class T> void pk2(Wp<T, k>) {}
template <class T, int... Ns> void pk3(Wp<T, Ns..., k>) {}
template <int... Ns> void pk4(V2<k, Ns..., 4>) {}
template <int... Ns> void pk5(V2<3, Ns..., 1 + 1>) {}
template <int... Ns> void pk6(V2<3, 4, Ns...>) {}
template <int... Ns> void pk7(P2<Ns..., k>) {}
template <class T> void pk8(D2<T>) {}
template <class T> void pk9(WA<T, k>) {}
template <class T> void pk10(WD<T, 1>) {}
template <class T> void pk11(WD<T, 1u>) {}

void use() {
  i1(It<int, true>{});
  i2(It<int, true>{});
  i3(It<It<int, true>, true>{});
  a1(A<int, e1>{});
  a2(A<int, e1>{});
  b1(B<int, F::b>{});
  b2(B<int, F::b>{});
  c1(C<int, nullptr>{});
  c2(C<int, nullptr>{});
  c3(C<int, &x>{});
  c4(C<int, &x>{});
  d1(D<int, x>{});
  d2(D<int, x>{});
  g1(G<int, P{1}>{});
  g2(G<int, P{1}>{});
  w1(W<int, 1>{});
  w2(W<int, 1>{});
  l1(L<int, 2>{});
  l2(L<int, 'a'>{});
  l3(L<int>{});
  z1<4>(Z<int, 4, 3>{});
  pk1<1, 2>(V<3, 1, 2>{});
  pk2(Wp<int, 3>{});
  pk3<int, 1>(Wp<int, 1, 3>{});
  pk4<1>(V2<3, 1, 4>{});
  pk5<1>(V2<3, 1, 2>{});
  pk6<1>(V2<3, 4, 1>{});
  pk7<1>(P2<1, 3>{});
  pk8(D2<int, 1>{});
  pk9(WA<int, 3>{});
  pk10(WD<int, 1>{});
  pk11(WD<int, 1>{});
}
} // namespace cv
