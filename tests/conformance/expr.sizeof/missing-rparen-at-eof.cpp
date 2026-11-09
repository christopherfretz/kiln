// [expr.sizeof]/1: `sizeof(` whose `)` is missing at the end of the
// file (WP-972: an ICE measuring the end-of-file location as the
// operand's closing token). The file deliberately ends without a newline
// (see expr.alignof/missing-rparen-at-eof.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:41 error "expected ')'"
//@ diag: 11:39 note "to match this '('"
//@ diag: 11:41 error "expected ';' after top level declarator"
using A = int;
int x = sizeof(A) + sizeof(A) * sizeof(A