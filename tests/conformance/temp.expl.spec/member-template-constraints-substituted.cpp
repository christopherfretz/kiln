// [temp.expl.spec]/16, [temp.over.link]/6: a member template of an
// explicitly specialized class template specialization is redeclared with
// its constraints as they are in the specialization: the template's own
// parameters at their depth there and the class's replaced by its
// arguments, in a requires-clause, a type-constraint or a trailing
// requires-clause (WP-958).
//@ std: c++20
//@ expect: compile-ok
template <typename T> concept C = sizeof(T) <= sizeof(long);
template <typename T> concept D = sizeof(T) >= 1;
template <typename T> struct A {
  template <typename U> requires C<U> void h(U);
  template <C U> void k(U);
  template <typename U> void m(U) requires (sizeof(T) + sizeof(U) > 2);
  template <typename U> void n(U) requires C<U> && D<U>;
  template <typename U, D V> void p(U, V) requires C<U>;
};
template <> template <typename U> requires C<U> void A<short>::h(U);
template <> template <C U> void A<short>::k(U);
template <> template <typename U> void A<short>::m(U) requires (sizeof(short) + sizeof(U) > 2);
template <> template <typename U> void A<short>::n(U) requires C<U> && D<U>;
template <> template <typename U, D V> void A<short>::p(U, V) requires C<U>;
