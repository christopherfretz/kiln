// [dcl.ref]/5, [dcl.array]/1, [temp.param]/14, [temp.inst]: a pointer to
// (or an array of) references formed by substitution is reported at the
// innermost written `*` or `[` (clang's `TreeTransform` rebuilds the type
// at its `TypeLoc` sigil: the first `*` of `T const *volatile *`, the
// return type's `*` of `T *h()` and `T *(*fp)(int)`), named after the
// declaration: a member, a parameter, a function, a typedef or alias, and
// a type parameter whose default is substituted (`'U'`). An invalid
// default (`R<T> *` with `R<T>` a reference, rejected in the definition)
// is dropped, so `S3<int>` lacks an argument. WP-437.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:42 error "'p' declared as a pointer to a reference of type 'int &'"
//@ diag: 33:6 note "in instantiation of template class 'S1<>' requested here"
//@ diag: 35:50 error "'U' declared as a pointer to a reference of type 'typename H<int>::type'"
//@ diag: 36:1 note "in instantiation of default argument for 'S2<int>' required here"
//@ diag: 38:35 error "'type name' declared as a pointer to a reference of type 'R<T>'"
//@ diag: 39:1 error "too few template arguments for class template 'S3'"
//@ diag: 38:45 note "template is declared here"
//@ diag: 40:38 error "'U' declared as a pointer to a reference of type 'int &'"
//@ diag: 41:1 note "in instantiation of default argument for 'S4<int &>' required here"
//@ diag: 42:31 error "'U' declared as array of references of type 'int &'"
//@ diag: 43:1 note "in instantiation of default argument for 'S5<int &>' required here"
//@ diag: 45:12 error "'p' declared as a pointer to a reference of type 'int &'"
//@ diag: 53:10 note "in instantiation of template class 'M<int &>' requested here"
//@ diag: 46:12 error "'g' declared as a pointer to a reference of type 'int &'"
//@ diag: 47:5 error "'h' declared as a pointer to a reference of type 'int &'"
//@ diag: 48:13 error "'P' declared as a pointer to a reference of type 'int &'"
//@ diag: 49:15 error "'Q' declared as a pointer to a reference of type 'int &'"
//@ diag: 50:5 error "'fp' declared as a pointer to a reference of type 'int &'"
//@ diag: 51:6 error "'a' declared as array of references of type 'int &'"
template <class T = int &> struct S1 { T *p; };
S1<> s1;
template <class T> struct H { using type = int &; };
template <class T, class U = typename H<T>::type *> struct S2 {};
S2<int> s2;
template <class T> using R = T &;
template <class T, class U = R<T> *> struct S3 {};
S3<int> s3;
template <class T, class U = T const *volatile *> struct S4 {};
S4<int &> s4;
template <class T, class U = T[3]> struct S5 {};
S5<int &> s5;
template <class T> struct M {
  void f(T *p);
  static T *g;
  T *h();
  typedef T *P;
  using Q = T *;
  T *(*fp)(int);
  T a[2];
};
M<int &> m;
