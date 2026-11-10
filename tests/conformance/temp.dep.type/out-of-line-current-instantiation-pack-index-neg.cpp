// [temp.over.link]/5, /7, [dcl.type.pack.index], [expr.prim.pack.index]
// (P2662R3): as clang, a pack-index-specifier that is the whole
// decl-specifier of an out-of-line member is not rebuilt in the current
// instantiation (`Ts...[C<Ts...>::M]` is not the in-class `Ts...[M]`),
// and an expression pack index whose rebuilt index is no longer
// value-dependent (`Ns...[D<Ns...>::K]`, `Ns...[sizeof(typename
// D<Ns...>::T)]`) keeps its written index, which is not the in-class
// converted one (WP-1169).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:53 error "return type of out-of-line definition of 'C::a' differs from that in the declaration"
//@ diag: 29:12 note "previous declaration is here"
//@ diag: 33:71 error "return type of out-of-line definition of 'C::b' differs from that in the declaration"
//@ diag: 30:21 note "previous declaration is here"
//@ diag: 42:54 error "return type of out-of-line definition of 'D::a' differs from that in the declaration"
//@ diag: 37:15 note "previous declaration is here"
//@ diag: 43:71 error "return type of out-of-line definition of 'D::b' differs from that in the declaration"
//@ diag: 38:23 note "previous declaration is here"
//@ diag: 44:60 error "return type of out-of-line definition of 'D::c' differs from that in the declaration"
//@ diag: 39:21 note "previous declaration is here"
//@ diag: 45:55 error "return type of out-of-line definition of 'D::d' differs from that in the declaration"
//@ diag: 40:23 note "previous declaration is here"
//@ requires: clang>=22
template <int N> struct I {};
template <class X> struct W {};
template <class... Ts> struct C {
  using T0 = Ts...[0];
  static constexpr int M = sizeof...(Ts) - 1;
  Ts...[M] a();
  Ts...[sizeof(T0)] b();
};
template <class... Ts> Ts...[C<Ts...>::M] C<Ts...>::a() { return {}; }
template <class... Ts> Ts...[sizeof(typename C<Ts...>::T0)] C<Ts...>::b() { return {}; }
template <int... Ns> struct D {
  using T = int;
  static constexpr int K = 1;
  I<Ns...[K]> a();
  I<Ns...[sizeof(T)]> b();
  W<char[Ns...[K]]> c();
  I<Ns...[sizeof(T)]> d();
};
template <int... Ns> I<Ns...[D<Ns...>::K]> D<Ns...>::a() { return {}; }
template <int... Ns> I<Ns...[sizeof(typename D<Ns...>::T)]> D<Ns...>::b() { return {}; }
template <int... Ns> W<char[Ns...[D<Ns...>::K]]> D<Ns...>::c() { return {}; }
template <int... Ns> I<Ns...[sizeof(long)]> D<Ns...>::d() { return {}; }
