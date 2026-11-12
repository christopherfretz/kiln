// [expr.unary.op]/1: the operand of unary `*` is a pointer to an object or
// function type, so `*pv` of a `void *` is ill-formed in C++, also as the
// operands of a conditional (C's `void` expression extension, WP-1285 and
// WP-1347, is C only).
//@ expect: compile-fail
//@ args: -Wno-unused-value
//@ strict-diags
//@ diag: 10:31 error "indirection not permitted on operand of type 'void *'"
//@ diag: 10:37 error "indirection not permitted on operand of type 'void *'"
void f(void *pv, int i) { i ? *pv : *pv; }
