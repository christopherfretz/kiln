// [temp.dep.type]/1, /6, [temp.over.link]/5: before the declarator-id of
// an out-of-line member of a class template, `C<X>::template f<A>` inside
// a signature expression names the member function or variable template
// of the current instantiation, so it is equivalent to the class's own
// `C::template f<A>` (clang's `RebuildExprInCurrentInstantiation` over a
// `DependentScopeDeclRefExpr` with template arguments): a call, a
// variable template-id, with a type or a constant argument, under
// `decltype` and `sizeof` (WP-1169).
//@ expect: run
//@ strict-diags
//@ stdout: 9 8 1 2 2 9 8
extern "C" int printf(const char*, ...);
template <int N> struct I { static constexpr int v = N; };
template <class X> struct C {
  template <class U> static constexpr int f() { return sizeof(U) + sizeof(X); }
  template <class U> static constexpr int vt = sizeof(U) * 2;
  template <int K> static constexpr int g() { return K; }
  template <int K> static constexpr int vk = K;
  static constexpr int N = sizeof(X);
  I<C::template f<char>() + N> a();
  I<C::template vt<X>> b();
  I<C::template g<1>()> c();
  I<C::template vk<2>> d();
  decltype(C::template f<short>()) e();
  I<sizeof(C::template f<int>()) + C::template vk<5>> h();
  I<C::template f<int>()> k();
};
template <class X> I<C<X>::template f<char>() + C<X>::N> C<X>::a() { return {}; }
template <class X> I<C<X>::template vt<X>> C<X>::b() { return {}; }
template <class X> I<C<X>::template g<1>()> C<X>::c() { return {}; }
template <class X> I<C<X>::template vk<2>> C<X>::d() { return {}; }
template <class X> decltype(C<X>::template f<short>()) C<X>::e() { return 2; }
template <class X> I<sizeof(C<X>::template f<int>()) + C<X>::template vk<5>> C<X>::h() {
  return {};
}
template <class X> I<C<X>::template f<int>()> C<X>::k() { return {}; }
int main() {
  C<int> c;
  printf("%d %d %d %d %d %d %d\n", decltype(c.a())::v, decltype(c.b())::v, decltype(c.c())::v,
         decltype(c.d())::v, c.e(), decltype(c.h())::v, decltype(c.k())::v);
}
