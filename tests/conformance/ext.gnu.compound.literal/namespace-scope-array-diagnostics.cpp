// Namespace-scope array compound literals ([GNU]; C99 6.5.2.5p3, clang's
// CheckForConstantInitializer): every element must be a constant
// initializer. The object of a non-const literal is not usable in a
// constant expression ([expr.const]/5.8: a read of an object whose
// lifetime did not begin within the evaluation).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 13:15 note "subexpression not valid in a constant expression"
//@ diag: 13:16 note "declared here"
//@ diag: 14:20 error "initializer element is not a compile-time constant"
int g();
static_assert(((int[]){1, 2})[1] == 2);
int *bad = (int[]){g()};
