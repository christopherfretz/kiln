// [expr.throw]/2, [class.abstract]/3, [except.throw]/5: the exception
// object may not be of abstract class type, and its destructor is
// potentially invoked, so it must be accessible; a pointer to such a
// class may be thrown (Boost.Exception exception_fail.cpp: `throw x` of
// a `boost::exception&`, WP-804).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:3 error "cannot throw an object of abstract type 'A'"
//@ diag: 15:31 note "unimplemented pure virtual method '~A' in 'A'"
//@ diag: 25:9 error "exception object of type 'C' has protected destructor"
//@ diag: 19:12 note "declared protected here"
//@ diag: 26:3 error "cannot throw an object of abstract type 'B'"
//@ diag: 17:25 note "unimplemented pure virtual method 'f' in 'B'"

struct A { protected: virtual ~A() = 0; };
inline A::~A() {}
struct B { virtual void f() = 0; };
struct C {
protected: ~C();
  friend void ok(C &);
};
void ok(C &c) { throw c; }
void f(A &a, B &b, C &c, A *pa) {
  throw a;
  throw c;
  throw b;
  throw pa;
}
