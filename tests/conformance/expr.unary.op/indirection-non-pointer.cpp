// [expr.unary.op]/1: * needs a pointer operand
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:9 error "indirection requires pointer operand ('int' invalid)"
int i;
int j = *i;
