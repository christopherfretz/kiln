// [temp.dep.type]/1, /6, [temp.over.link]/5, [dcl.type.pack.index],
// [expr.prim.pack.index] (P2662R3): a pack index nested in the signature
// of an out-of-line member of a class template is rebuilt in the current
// instantiation without expanding its pack: `C<Ts...>::N` in the index
// names the member, and `D<Ns...>` in it keeps its expansion. A type
// pack index inside a template-id and an expression pack index whose
// rebuilt index is still value-dependent are the in-class ones (clang's
// `TreeTransform` under `RebuildDeclaratorInCurrentInstantiation`;
// WP-1169). clang 19 mis-handles pack indexing here.
//@ expect: run
//@ strict-diags
//@ requires: clang>=22
//@ stdout: 4 8 4 4 1 5 6 9
extern "C" int printf(const char*, ...);
template <int N> struct I { static constexpr int v = N; };
template <class X> struct W { static constexpr int v = sizeof(X); };
template <class... Ts> struct C {
  using T0 = Ts...[0];
  static constexpr int N = 1;
  static constexpr int M = sizeof...(Ts) - 1;
  W<Ts...[N]> a();
  W<Ts...[M]> b();
  W<Ts...[sizeof(T0)]> c();
  I<sizeof(Ts...[N])> d();
  W<Ts...[sizeof(T0) - 1]> e();
};
template <class... Ts> W<Ts...[C<Ts...>::N]> C<Ts...>::a() { return {}; }
template <class... Ts> W<Ts...[C<Ts...>::M]> C<Ts...>::b() { return {}; }
template <class... Ts> W<Ts...[sizeof(typename C<Ts...>::T0)]> C<Ts...>::c() { return {}; }
template <class... Ts> I<sizeof(Ts...[C<Ts...>::N])> C<Ts...>::d() { return {}; }
template <class... Ts> W<Ts...[sizeof(typename C<Ts...>::T0) - 1]> C<Ts...>::e() {
  return {};
}
template <int... Ns> struct D {
  static constexpr int K = 1;
  static constexpr int M = sizeof...(Ns) - 1;
  I<Ns...[M]> a();
  I<Ns...[sizeof(int)] + K> b();
  I<Ns...[sizeof(W<D*>) - 1]> c();
};
template <int... Ns> I<Ns...[D<Ns...>::M]> D<Ns...>::a() { return {}; }
template <int... Ns> I<Ns...[sizeof(int)] + D<Ns...>::K> D<Ns...>::b() { return {}; }
template <int... Ns> I<Ns...[sizeof(W<D<Ns...>*>) - 1]> D<Ns...>::c() { return {}; }
int main() {
  C<char, int, short, long> c;
  D<9, 8, 7, 6, 5> d;
  printf("%d %d %d %d %d %d %d %d\n", decltype(c.a())::v, decltype(c.b())::v, decltype(c.c())::v,
         decltype(c.d())::v, decltype(c.e())::v, decltype(d.a())::v, decltype(d.b())::v,
         decltype(d.c())::v);
}
