// [temp.arg.nontype]/1, [temp.type]/1: a non-dependent non-type argument
// of a dependent class template-id is converted to its parameter's type,
// in the elaborated spelling (`struct It<T, S{}>`) as in the simple one:
// a value with no implicit conversion, a narrowing conversion, the
// address `&x` for a reference parameter, `x` or `0` for a pointer one
// are errors at the argument (WP-3321, WP-3322).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
// clang 22 spelling of elaborated-dependent-template-id-conversion.cpp.
//@ diag: 42:9 error "value of type 'S' is not implicitly convertible to 'bool'"
//@ diag: 37:25 note "template parameter is declared here"
//@ diag: 43:16 error "value of type 'S' is not implicitly convertible to 'bool'"
//@ diag: 37:25 note "template parameter is declared here"
//@ diag: 44:8 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 38:24 note "template parameter is declared here"
//@ diag: 45:15 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 38:24 note "template parameter is declared here"
//@ diag: 46:10 error "value of type 'int *' is not implicitly convertible to 'int &'"
//@ diag: 40:25 note "template parameter is declared here"
//@ diag: 47:17 error "value of type 'int *' is not implicitly convertible to 'int &'"
//@ diag: 40:25 note "template parameter is declared here"
//@ diag: 48:10 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 39:25 note "template parameter is declared here"
//@ diag: 49:16 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 39:25 note "template parameter is declared here"
//@ diag: 50:10 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 39:25 note "template parameter is declared here"
//@ diag: 51:17 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 39:25 note "template parameter is declared here"
//@ diag: 52:16 error "value of type 'std::nullptr_t' is not implicitly convertible to 'bool'"
//@ diag: 37:25 note "template parameter is declared here"
//@ diag: 53:23 error "value of type 'std::nullptr_t' is not implicitly convertible to 'bool'"
//@ diag: 37:25 note "template parameter is declared here"
int x;
struct S {};
template <class T, bool B> struct It {};
template <class T, int N> struct Q {};
template <class T, int *p> struct Ptr {};
template <class T, int &r> struct Ref {};
template <class T> struct C {
  It<T, S{}> *a1;
  struct It<T, S{}> *a2;
  Q<T, 1.5> *b1;
  struct Q<T, 1.5> *b2;
  Ref<T, &x> *c1;
  struct Ref<T, &x> *c2;
  Ptr<T, x> *d1;
  class Ptr<T, x> *d2;
  Ptr<T, 0> *e1;
  struct Ptr<T, 0> *e2;
  friend It<T, nullptr>;
  friend struct It<T, nullptr>;
};
