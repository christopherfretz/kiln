// [expr.log.and]/1: -Wno-constant-logical-operand silences the clang
// "use of logical '&&' with constant operand" warning (and its notes).
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wno-constant-logical-operand
int f(int x) { return (x && 5) + (x || 2) + (x && 0); }
