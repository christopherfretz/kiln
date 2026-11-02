//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:18 error "'const' is invalid in friend declarations"
//@ diag: 11:24 error "'volatile' is invalid in friend declarations"
//@ diag: 13:39 error "'const' is invalid in friend declarations"
// [class.friend]/3: a friend-type-specifier is a simple-, elaborated- or
// typename-specifier, so a friend type declaration takes no cv-qualifier.
// Each is reported at the qualifier and the friend is still declared (no
// access errors follow); a cv-qualified typedef is fine.
struct X;
class A { friend const volatile X; int s = 1; };
struct X { int g(A a) { return a.s; } };
template <class T> class B { friend T const; int s = 2; };
struct Y { int g(B<Y> b) { return b.s; } };
typedef const X CX;
class D { friend CX; };
