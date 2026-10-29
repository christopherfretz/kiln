// [dcl.type.auto.deduct]/4: `auto *` cannot deduce from a pointer to member.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:7 error "variable 'e' with type 'auto *' has incompatible initializer of type 'int S::*'"
struct S { int m; };
auto *e = &S::m;
