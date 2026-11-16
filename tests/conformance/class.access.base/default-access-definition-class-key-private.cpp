// [class.access.base]/2: a base-specifier without an access-specifier in a
// class defined with `class` is private even when the class was first
// declared with `struct`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:21 error "cannot cast 'A' to its private base class 'B'"
//@ diag: 11:11 note "implicitly declared private here"

struct A;
struct B { int m; };
class A : B {};
B *f(A *a) { return a; }
