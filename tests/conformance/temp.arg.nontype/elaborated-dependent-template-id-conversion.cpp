// [temp.arg.nontype]/1, [temp.type]/1: a non-dependent non-type argument
// of a dependent class template-id is converted to its parameter's type,
// in the elaborated spelling (`struct It<T, S{}>`) as in the simple one:
// a value with no implicit conversion, a narrowing conversion, the
// address `&x` for a reference parameter, `x` or `0` for a pointer one
// are errors at the argument (WP-3321, WP-3322).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
// clang 19 spelling; elaborated-dependent-template-id-conversion-clang22.cpp
// has clang 22's, with the parameter notes.
//@ diag: 31:9 error "value of type 'S' is not implicitly convertible to 'bool'"
//@ diag: 32:16 error "value of type 'S' is not implicitly convertible to 'bool'"
//@ diag: 33:8 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 34:15 error "conversion from 'double' to 'int' is not allowed in a converted constant expression"
//@ diag: 35:10 error "value of type 'int *' is not implicitly convertible to 'int &'"
//@ diag: 36:17 error "value of type 'int *' is not implicitly convertible to 'int &'"
//@ diag: 37:10 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 38:16 error "value of type 'int' is not implicitly convertible to 'int *'"
//@ diag: 39:10 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 40:17 error "conversion from 'int' to 'int *' is not allowed in a converted constant expression"
//@ diag: 41:16 error "value of type 'std::nullptr_t' is not implicitly convertible to 'bool'"
//@ diag: 42:23 error "value of type 'std::nullptr_t' is not implicitly convertible to 'bool'"
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
