// [temp.over.link]/5, [temp.dep.type]/6, Itanium C++ ABI 5.1.5: a member
// function template of a class template, defined out of line with
// `C<X>::template f<U>` and `__typeof__(C<X>::N + u)` in its return
// type, is the in-class declaration, whose return type is mangled
// (`I<C::template f<U>() + N>` as `IXplclsrNS_IT_EE1fIT_EEEL_ZNS_1NEEE`
// style): the rebuilt template-id and `typeof` operand mangle as the
// class wrote them (WP-1169).
//@ expect: ir
//@ args: -std=c++26 -ftrivial-auto-var-init=uninitialized
template <int N> struct I {};
template <class X> struct C {
  template <class U> static constexpr int f() { return sizeof(U) + sizeof(X); }
  static constexpr int N = sizeof(X);
  template <class U> I<C::template f<U>() + N> g(U);
  template <class U> __typeof__(U() + N) h(U u);
};
template <class X>
template <class U>
I<C<X>::template f<U>() + C<X>::N> C<X>::g(U) {
  return {};
}
template <class X>
template <class U>
__typeof__(U() + C<X>::N) C<X>::h(U u) {
  return u;
}
void use(C<int> c) {
  c.g('a');
  c.h(2L);
}
