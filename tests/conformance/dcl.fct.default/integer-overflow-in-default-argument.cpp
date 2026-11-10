// [dcl.fct.default]/5, [expr.pre]/4: clang's -Winteger-overflow trial
// (EvaluateForOverflow) evaluates a call's arguments, default ones
// included, before the callee, so an overflow in a default argument warns
// again at each use under an operator, whatever the callee. A default
// argument that holds an immediate invocation is rebuilt and completed at
// each use (BuildCXXDefaultArgExpr, CheckCompletedExpr), so it warns there
// once more and a failing invocation in it is reported before the
// enclosing full-expression's warnings. kiln reads the invocation's
// stored value instead of evaluating it again ([expr.const]/16, WP-1171).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:26 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 31:36 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:31 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 30:26 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 31:36 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 30:26 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:31 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 32:31 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 29:38 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 35:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 43:51 note "in the default initializer of 'x'"
//@ diag: 35:9 note "declared here"
//@ diag: 28:38 note "division by zero"
//@ diag: 35:17 note "in call to 'bad(0)'"
//@ diag: 43:44 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
consteval int id(int x) { return x; }
consteval int bad(int x) { return 10 / x; }
constexpr int cinc(int x) { return x + 1; }
int f(int x = 2147483647 + 1);
constexpr int k(int x = 2147483647 + 1) { return x; }
int ic(int x = id(2147483647) + 1);
int cc(int x = cinc(2147483647));
int ok(int x = id(2147483647));
int def(int x = bad(0));
void top() { f(); k(); }
int nonconstexpr_callee() { return f() + 1; }
int constexpr_callee() { return k() + 1; }
int nested(int y) { return y + (f() & 1); }
int valued_invocation() { return ok() + 1; }
int rebuilt_default() { return ic() + 1; }
int constexpr_call_in_default() { return cc() + 1; }
int failing_default() { return (2147483647 + 1) + def(); }
