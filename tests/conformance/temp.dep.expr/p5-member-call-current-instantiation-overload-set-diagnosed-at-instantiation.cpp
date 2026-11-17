// [temp.res.general]/6, [temp.dep.expr]/5: a call through `this` of the
// current instantiation that overload resolution at definition finds
// ambiguous, not viable or deleted is not diagnosed at definition (`u1`
// is never instantiated): the call stays type-dependent and is diagnosed
// when instantiated, with clang 22's errors and notes. A call that
// resolves at definition (`f5`: `tm(int)` by a conversion) is not an
// error. GCC 16.1 also diagnoses these only at instantiation. WP-1609.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:21 error "call to member function 'ov' is ambiguous"
//@ diag: 42:5 note "in instantiation of member function 'A<int>::f1' requested here"
//@ diag: 27:8 note "candidate function"
//@ diag: 28:8 note "candidate function"
//@ diag: 34:15 error "no matching member function for call to 'ov'"
//@ diag: 43:5 note "in instantiation of member function 'A<int>::f2' requested here"
//@ diag: 27:8 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
//@ diag: 28:8 note "candidate function not viable: no known conversion from 'const char[2]' to 'long' for 1st argument"
//@ diag: 35:15 error "call to deleted member function 'dl'"
//@ diag: 44:5 note "in instantiation of member function 'A<int>::f3' requested here"
//@ diag: 29:8 note "candidate function has been explicitly deleted"
//@ diag: 30:8 note "candidate function"
//@ diag: 36:21 error "no matching member function for call to 'ov'"
//@ diag: 45:5 note "in instantiation of member function 'A<int>::f4' requested here"
//@ diag: 27:8 note "candidate function not viable: 'this' argument has type 'const A<int>', but method is not marked const"
//@ diag: 28:8 note "candidate function not viable: 'this' argument has type 'const A<int>', but method is not marked const"
template <class T> struct A {
  void ov(int);
  void ov(long);
  void dl(int) = delete;
  void dl(long);
  template <class U> void tm(U *);
  void tm(int);
  void f1() { this->ov(1.0); }
  void f2() { ov("x"); }
  void f3() { dl(1); }
  void f4() const { ov(1); }
  void f5() { tm(1.0); }
  void u1() { this->ov(1.0); ov(""); dl(1); tm(1.0); }
};
void use() {
  A<int> a;
  a.f1();
  a.f2();
  a.f3();
  a.f4();
  a.f5();
}
