// [dcl.type.decltype]/1, [expr.context]/1: the operand is unevaluated: no
// odr-use, so a declared-only function is fine.
//@ expect: run
//@ strict-diags
int undefined_fn();
extern int undefined_var;
decltype(undefined_fn()) a = 1;
decltype(undefined_var) b = 2;
int main() { return a + b - 3; }
