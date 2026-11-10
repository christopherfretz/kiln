// [temp.over.link]/5, /7, [temp.dep.type]/6: a template-id in the
// signature of an out-of-line member, rebuilt in the current
// instantiation as the class's `C::template f<A>`, is still not
// equivalent to the unqualified `f<A>` the class wrote, nor to another
// argument (WP-1169).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:54 error "return type of out-of-line definition of 'C::a' differs from that in the declaration"
//@ diag: 20:16 note "previous declaration is here"
//@ diag: 26:62 error "return type of out-of-line definition of 'C::b' differs from that in the declaration"
//@ diag: 21:24 note "previous declaration is here"
//@ diag: 27:55 error "return type of out-of-line definition of 'C::c' differs from that in the declaration"
//@ diag: 22:28 note "previous declaration is here"
//@ diag: 28:50 error "return type of out-of-line definition of 'C::d' differs from that in the declaration"
//@ diag: 23:24 note "previous declaration is here"
template <int N> struct I {};
template <class X> struct C {
  template <class U> static constexpr int f() { return sizeof(U); }
  template <int K> static constexpr int vk = K;
  I<f<char>()> a();
  decltype(f<short>()) b();
  I<C::template f<char>()> c();
  I<C::template vk<2>> d();
};
template <class X> I<C<X>::template f<char>()> C<X>::a() { return {}; }
template <class X> decltype(C<X>::template f<short>()) C<X>::b() { return 0; }
template <class X> I<C<X>::template f<short>()> C<X>::c() { return {}; }
template <class X> I<C<X>::template vk<3>> C<X>::d() { return {}; }
