// [temp.dep.type]/1, /6, [temp.over.link]/5: before the declarator-id of
// an out-of-line member of a class template, a `typename C<X>::T` or a
// `C<X>::N` inside an expression names the member of the current
// instantiation, so the expression is equivalent to the in-class
// declaration's: a `decltype` operand, a value-dependent array bound, a
// `noexcept` operand of a function type, an expression template argument,
// and `sizeof`/`alignof`/`noexcept` inside them (clang's
// `RebuildExprInCurrentInstantiation`).
//@ expect: run
//@ strict-diags
//@ stdout: 7 5 4 5 1 1 5 8 4 1 8
//@ stdout: 3 0 4
extern "C" int printf(const char*, ...);
template <class X> struct W { static constexpr int v = sizeof(X); };
template <class F> struct NX { static constexpr int v = 0; };
template <class R> struct NX<R() noexcept> { static constexpr int v = 1; };
template <int N> struct I { static constexpr int v = N; };
template <bool B> struct BB { static constexpr int v = B; };
template <class X> struct C {
  using T = X;
  static constexpr int N = sizeof(X);
  static constexpr bool B = sizeof(X) > 2;
  T v = 7;
  decltype(T()) d();
  decltype(N + 1) dn();
  W<char[N]> arr();
  W<char[sizeof(T) + 1]> arr2();
  NX<void() noexcept(B)> nx();
  NX<void() noexcept(sizeof(T) > 2)> nx2();
  I<N + 1> ta();
  I<sizeof(T) * 2> ts();
  I<alignof(T)> tal();
  BB<noexcept(T())> tne();
  W<decltype(sizeof(T))> wd();
};
template <class X> decltype(typename C<X>::T()) C<X>::d() { return v; }
template <class X> decltype(C<X>::N + 1) C<X>::dn() { return N + 1; }
template <class X> W<char[C<X>::N]> C<X>::arr() { return {}; }
template <class X> W<char[sizeof(typename C<X>::T) + 1]> C<X>::arr2() { return {}; }
template <class X> NX<void() noexcept(C<X>::B)> C<X>::nx() { return {}; }
template <class X> NX<void() noexcept(sizeof(typename C<X>::T) > 2)> C<X>::nx2() { return {}; }
template <class X> I<C<X>::N + 1> C<X>::ta() { return {}; }
template <class X> I<sizeof(typename C<X>::T) * 2> C<X>::ts() { return {}; }
template <class X> I<alignof(typename C<X>::T)> C<X>::tal() { return {}; }
template <class X> BB<noexcept(typename C<X>::T())> C<X>::tne() { return {}; }
template <class X> W<decltype(sizeof(typename C<X>::T))> C<X>::wd() { return {}; }
int main() {
  C<int> c;
  printf("%d %d %d %d %d %d %d %d %d %d %d\n", c.d(), c.dn(), decltype(c.arr())::v,
         decltype(c.arr2())::v, decltype(c.nx())::v, decltype(c.nx2())::v, decltype(c.ta())::v,
         decltype(c.ts())::v, decltype(c.tal())::v, decltype(c.tne())::v, decltype(c.wd())::v);
  C<short> s;
  printf("%d %d %d\n", s.dn(), decltype(s.nx())::v, decltype(s.ts())::v);
}
