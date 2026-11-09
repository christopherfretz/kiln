// [dcl.fct]/17: types shall not be defined in parameter types. Covers an
// in-class member declaration, the parameter clause of out-of-line member
// and constructor definitions of a complete class (the tag is never entered
// into the class), a scoped enumeration, a lambda and a requires-expression
// parameter list. Each is diagnosed once, with no follow-on mismatch error.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:26 error "'T' cannot be defined in a parameter type"
//@ diag: 17:16 error "'E2' cannot be defined in a parameter type"
//@ diag: 18:13 error "'S2' cannot be defined in a parameter type"
//@ diag: 19:33 error "'E3' cannot be defined in a parameter type"
//@ diag: 20:20 error "'L' cannot be defined in a parameter type"
//@ diag: 21:47 error "'R' cannot be defined in a parameter type"
struct M { void h(struct T { int a; } t); };
struct A { void f(int); }; struct B { B(int); };
void A::f(enum E2 { x2 }) {}
B::B(struct S2 { int b; } s) {}
namespace N { void k(enum class E3 { y3 }); }
auto l = [](struct L { int y; } p) { return 0; };
template <class T> concept C = requires(union R { int z; } r) { r; };
int after = sizeof(A) + sizeof(B);
