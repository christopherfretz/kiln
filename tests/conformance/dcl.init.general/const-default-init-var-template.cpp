// [dcl.init.general]/8, [temp.inst]/7: an implicit variable template
// specialization instantiated from a definition without an initializer is
// default-initialized, so a const one of a type without a user-provided
// default constructor is an error at the definition, noted at the use:
// the primary template, a partial specialization, and the out-of-line
// definition of a static data member template. [temp.expl.spec]/14: an
// explicit specialization of a static data member (template) without an
// initializer is a declaration, and an out-of-class member partial
// specialization without one is not instantiated as a definition.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:28 error "default initialization of an object of const type 'const int'"
//@ diag: 20:9 note "in instantiation of variable template specialization 'c<int>' requested here"
//@ diag: 24:30 error "default initialization of an object of const type 'const int'"
//@ diag: 32:33 error "default initialization of an object of const type 'const int'"
//@ diag: 33:33 error "default initialization of an object of const type 'const int'"
//@ diag: 35:31 error "default initialization of an object of const type 'const int'"
//@ diag: 36:12 note "in instantiation of static data member 'M::v<int>' requested here"
template <class T> const T c;
int b = c<int>;
struct E {};
E e = c<E>;
template <class T> const int p = 0;
template <class T> const int p<T *>;
int q = p<int *>;
struct M {
  template <class T> static const int sm;
  template <class T> static const int sm2 = 0;
  template <class T> static const T v;
};
template <> const int M::sm<int>;
template <class T> const int M::sm<T *>;
template <class T> const int M::sm2<T *>;
int u = M::sm2<int *>;
template <class T> const T M::v;
int w = M::v<int>;
template <class T> struct C {
  static const int x;
  template <class U> static const int y;
};
template <> const int C<int>::x;
template <> template <> const int C<int>::y<int>;
