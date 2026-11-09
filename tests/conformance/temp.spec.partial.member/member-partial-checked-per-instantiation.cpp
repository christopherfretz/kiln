// [temp.spec.partial.member], [temp.spec.partial.general]/8-9: a partial
// specialization of a member template of a class template is checked when
// the enclosing class is instantiated, as clang does: "not more specialized
// than the primary template" only there (its arguments are dependent at the
// definition), unusable (non-deducible) at the definition and again in each
// instantiation. WP-1053.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:38 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 42:28 note "non-deducible template parameter 'W'"
//@ diag: 44:32 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 44:22 note "non-deducible template parameter 'T'"
//@ diag: 37:25 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 54:8 note "in instantiation of template class 'O<int>' requested here"
//@ diag: 36:25 note "template is declared here"
//@ diag: 39:36 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 38:36 note "template is declared here"
//@ diag: 42:38 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 42:28 note "non-deducible template parameter 'W'"
//@ diag: 44:32 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 44:22 note "non-deducible template parameter 'T'"
//@ diag: 37:25 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 55:9 note "in instantiation of template class 'O<long>' requested here"
//@ diag: 36:25 note "template is declared here"
//@ diag: 39:36 error "class template partial specialization is not more specialized than the primary template"
//@ diag: 38:36 note "template is declared here"
//@ diag: 42:38 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 42:28 note "non-deducible template parameter 'W'"
//@ diag: 44:32 error "class template partial specialization contains a template parameter that cannot be deduced; this partial specialization will never be used"
//@ diag: 44:22 note "non-deducible template parameter 'T'"
template <class T> concept C = sizeof(T) > 1;
template <class T> concept D = C<T> && sizeof(T) > 2;
template <class U> struct O {
  // Checked per instantiation only.
  template <D T> struct K {};
  template <C T> struct K<T> {};
  template <int N, char C2> struct X {};
  template <char C2, int N> struct X<N, C2> {};
  // Checked at the definition and again per instantiation.
  template <class T, class V> struct A {};
  template <class T, class W> struct A<T, U> {};
  template <class... Ts> struct F {};
  template <class... T> struct F<T..., U> {};
  // Valid.
  template <class T, class V> struct L {};
  template <class T> struct L<T, U> {};
  template <class T, U V> struct P {};
  template <class T> struct P<T, 0> {};
  template <class T> struct Q {};
  template <C T> struct Q<T> {};
};
template <class U> template <class T> struct O<U>::Q<T *> {};
O<int> o;
O<long> o2;
