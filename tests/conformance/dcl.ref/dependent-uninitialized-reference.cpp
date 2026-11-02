// [dcl.ref]/5, [temp.res.general]/6: a variable definition of dependent
// reference type with no initializer (`T &r;`) is reported at the template
// definition, as clang's `ActOnUninitializedDecl` checks a reference type
// before it stops at a dependent one, and again, with the instantiation
// note, for each instantiation. A dependent type that is not syntactically a
// reference (`T x;`, `typename T::type &` aside) waits for instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:6 error "declaration of reference variable 'r' requires an initializer"
//@ diag: 26:8 error "declaration of reference variable 's' requires an initializer"
//@ diag: 30:17 error "declaration of reference variable 'q' requires an initializer"
//@ diag: 33:49 error "declaration of reference variable 't' requires an initializer"
//@ diag: 25:6 error "declaration of reference variable 'r' requires an initializer"
//@ diag: 36:3 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 26:8 error "declaration of reference variable 's' requires an initializer"
//@ diag: 25:6 error "declaration of reference variable 'r' requires an initializer"
//@ diag: 37:3 note "in instantiation of function template specialization 'f<int &>' requested here"
//@ diag: 26:8 error "declaration of reference variable 's' requires an initializer"
//@ diag: 27:5 error "declaration of reference variable 'x' requires an initializer"
//@ diag: 30:17 error "declaration of reference variable 'q' requires an initializer"
//@ diag: 38:12 note "in instantiation of member function 'A<int>::g' requested here"
//@ diag: 33:49 error "declaration of reference variable 't' requires an initializer"
//@ diag: 39:3 note "in instantiation of function template specialization 'h<B>' requested here"
template <class T> void f() {
  T &r;
  int &s;
  T x;
}
template <class T> struct A {
  void g() { T &q, &p = q; }
  static T &m;
};
template <class T> void h() { typename T::type &t; }
struct B { using type = int; };
void use() {
  f<int>();
  f<int &>();
  A<int>().g();
  h<B>();
}
