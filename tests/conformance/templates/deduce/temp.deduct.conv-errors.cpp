// [temp.deduct.conv], [over.match.conv]: deduction failures of
// conversion function templates are noted; an explicit template is not
// a candidate for copy-initialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:5 error "no viable conversion from 'A' to 'int'"
//@ diag: 15:22 note "candidate template ignored: could not match 'type-parameter-0-0 *' against 'int'"
//@ diag: 21:5 error "no viable conversion from 'B' to 'int'"
//@ diag: 19:31 note "explicit conversion function is not a candidate"
//@ diag: 30:9 error "no viable conversion from 'D' to 'double'"
//@ diag: 28:22 note "candidate template ignored: cannot deduce a type for 'T' that would make 'const T' equal 'double'"
//@ requires: clang<22
// clang 19 spelling; temp.deduct.conv-errors-clang22.cpp has clang 22's (WP-377).
struct A {
  template <class T> operator T*() const;
};
int x = A();
struct B {
  template <class T> explicit operator T() const;
};
int y = B();
struct C {
  template <class T> operator T() const;
  template <class T> operator T*() const;
};
int *p = C();
struct D {
  template <class T> operator const T&() const;
};
double& r = D();
