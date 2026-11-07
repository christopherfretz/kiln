// [namespace.udecl]/10: two using-declarators in a class naming the same
// member through the same class (a typedef of it included) are an error;
// through a different class they are not (p10-same-member-through-other-class-run.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:38 error "redeclaration of using declaration"
//@ diag: 10:25 note "previous using declaration"
struct A { void f(); };
typedef A AA;
struct B : A { using A::f; using AA::f; };
