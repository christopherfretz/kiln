// [class.abstract]/3, [dcl.init.ref]/5.4.1: binding a const reference to an
// abstract class from an object of an unrelated class through a converting
// constructor creates a temporary of the abstract class, so it is ill-formed;
// a conversion function to a reference to it creates none.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:26 error "allocating an object of abstract class type 'A'"
//@ diag: 12:16 note "unimplemented pure virtual method 'f' in 'A'"
struct S {};
struct A {
  A(const S &);
  virtual void f() const = 0;
};
struct T {
  operator const A &() const;
};
void g(const A &);
void ok(const T &t) { g(t); }
void bad(const S &s) { g(s); }
