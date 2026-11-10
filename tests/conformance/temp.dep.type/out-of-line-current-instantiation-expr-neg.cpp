// [temp.over.link]/5, /7, [temp.dep.type]/6: an expression in the
// signature of an out-of-line member, rebuilt in the current
// instantiation, still has to be equivalent to the in-class one: another
// constant, or an operand naming a member typedef (instantiation-dependent
// there, not in the class), is a different return type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:48 error "return type of out-of-line definition of 'D::a' differs from that in the declaration"
//@ diag: 18:19 note "previous declaration is here"
//@ diag: 25:41 error "return type of out-of-line definition of 'D::b' differs from that in the declaration"
//@ diag: 19:12 note "previous declaration is here"
//@ diag: 26:61 error "return type of out-of-line definition of 'D::d' differs from that in the declaration"
//@ diag: 20:23 note "previous declaration is here"
template <int N> struct I {};
template <class B> struct D : B {
  static constexpr int N = 1;
  using T = int;
  decltype(N + 1) a();
  I<N + 1> b();
  decltype(sizeof(T)) d();
  decltype(D<B>::M) c();
  I<sizeof(T)> e();
};
template <class B> decltype(D<B>::N + 2) D<B>::a() { return 0; }
template <class B> I<D<B>::N + 2> D<B>::b() { return {}; }
template <class B> decltype(sizeof(typename D<B>::T)) D<B>::d() { return 0; }
template <class B> decltype(D<B>::M) D<B>::c() { return 0; }
template <class B> I<sizeof(int)> D<B>::e() { return {}; }
