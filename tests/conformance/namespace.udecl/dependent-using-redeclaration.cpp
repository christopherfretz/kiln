// [namespace.udecl]/10: in a class, a using-declarator cannot be repeated.
// With a dependent qualifier the repetition is diagnosed at the template
// definition: the same kind (`typename` or not) through the same qualifier.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 17:15 error "redeclaration of using declaration"
//@ diag: 16:15 note "previous using declaration"
//@ diag: 20:24 error "redeclaration of using declaration"
//@ diag: 19:24 note "previous using declaration"
//@ diag: 24:12 error "redeclaration of using declaration"
//@ diag: 23:12 note "previous using declaration"
//@ diag: 29:17 error "redeclaration of using declaration"
//@ diag: 28:17 note "previous using declaration"
template<class T> struct V { int i, j; using type = int; };
template<class T> struct D : V<T> {
  using V<T>::i;
  using V<T>::i;
  using V<T>::j;
  using typename V<T>::type;
  using typename V<T>::type;
};
template<class T> struct G : T {
  using T::x;
  using T::x;
};
template<class T> struct K {
  template<class U> struct In : V<U> {
    using V<U>::i;
    using V<U>::i;
  };
};
