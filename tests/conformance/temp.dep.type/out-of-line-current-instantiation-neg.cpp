// [temp.dep.type]/1, [dcl.meaning.general]/2: a qualified name that is not
// the declared member of the current instantiation (another typedef of it,
// another partial specialization's member, an alias template of another
// member, another nested class) still makes the out-of-line definition
// match no declaration (WP-553).
// clang 19 spelling; the -clang22 sibling has clang 22's.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 23:34 error "out-of-line definition of 'at' does not match any declaration in 'vec<2, type-parameter-0-0>'"
//@ diag: 24:34 error "out-of-line definition of 'at' does not match any declaration in 'vec<2, type-parameter-0-0>'"
//@ diag: 25:62 error "return type of out-of-line definition of 'vec<2, type-parameter-0-0>::size' differs from that in the declaration"
//@ diag: 21:121 note "previous declaration is here"
//@ diag: 31:76 error "return type of out-of-line definition of 'base::get' differs from that in the declaration"
//@ diag: 29:26 note "previous declaration is here"
//@ diag: 41:86 error "redefinition of 'noone' with a different type: 'typename policy::apply<HP>::type::other' vs 'owner'"
//@ diag: 37:20 note "previous declaration is here"
template <class T> struct ident { using type = T; };
template <class T> using ident_t = typename ident<T>::type;
template <int L, class T> struct vec;
template <class T> struct vec<2, T> { typedef int length_type; typedef long other_type; T& at(length_type); length_type size(); T x; };
template <class T> struct vec<3, T> { typedef int length_type; };
template <class T> T& vec<2, T>::at(typename vec<2, T>::other_type) { return x; }
template <class T> T& vec<2, T>::at(typename vec<3, T>::length_type) { return x; }
template <class T> typename vec<2, T>::other_type vec<2, T>::size() { return 2; }
template <class T, class C> struct base {
  using element_type = T;
  using ctx = C;
  ident_t<element_type>* get() const;
};
template <class T, class C> ident_t<typename base<T, C>::ctx>* base<T, C>::get() const { return nullptr; }
struct policy {
  template <class HP> struct apply {
    struct type {
      struct owner { int v; };
      struct other { int v; };
      static owner noone;
    };
  };
};
template <class HP> typename policy::apply<HP>::type::other policy::apply<HP>::type::noone = {};
