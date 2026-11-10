// [temp.over.link]/5, /7, [temp.dep.type]/6: a GNU `typeof` operand in
// the signature of an out-of-line member, rebuilt in the current
// instantiation, still has to be equivalent to the in-class one: another
// array bound or another operand is another type, while
// `sizeof(typename C<X>::T)` is the in-class `sizeof(T)` (WP-1169).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:59 error "return type of out-of-line definition of 'C::b' differs from that in the declaration"
//@ diag: 17:26 note "previous declaration is here"
//@ diag: 22:56 error "return type of out-of-line definition of 'C::c' differs from that in the declaration"
//@ diag: 18:23 note "previous declaration is here"
template <class X> struct W {};
template <class X> struct C {
  using T = int;
  static constexpr int N = sizeof(X);
  __typeof__(sizeof(T)) a();
  W<__typeof__(char[N])> b();
  __typeof__(X() + N) c();
};
template <class X> __typeof__(sizeof(typename C<X>::T)) C<X>::a() { return 0; }
template <class X> W<__typeof__(char[C<X>::N + 1])> C<X>::b() { return {}; }
template <class X> __typeof__(X() + C<X>::N + 1) C<X>::c() { return {}; }
