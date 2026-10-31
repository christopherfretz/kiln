// [temp.deduct.conv], [over.match.conv]: deduction failures of
// conversion function templates are noted; an explicit template is not
// a candidate for copy-initialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:5 error "no viable conversion from 'A' to 'int'"
//@ diag: 13:22 note "candidate template ignored: could not match 'type-parameter-0-0 *' against 'int'"
//@ diag: 19:5 error "no viable conversion from 'B' to 'int'"
//@ diag: 17:31 note "explicit conversion function is not a candidate"
//@ diag: 28:9 error "no viable conversion from 'D' to 'double'"
//@ diag: 26:22 note "candidate template ignored: cannot deduce a type for 'T' that would make 'const T' equal 'double'"
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
