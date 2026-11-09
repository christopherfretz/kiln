// [expr.const]/16: an immediate invocation is evaluated once, at the end
// of its full-expression. The completion checks that fold it first (the
// overflow and implicit-conversion warnings, clang's CheckCompletedExpr)
// read the result evaluated before them (WP-1129): a failing invocation
// is reported exactly once, after the full-expression's warnings, in an
// addition, a narrowing return, a conditional's condition, a comparison
// and a negation; a default argument's is reported once per use
// ([dcl.fct.default]/5). An overflow through a successful invocation's
// stored value is still warned.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:29 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 44:29 note "in call to 'bad(0)'"
//@ diag: 45:22 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 45:22 note "in call to 'bad(0)'"
//@ diag: 46:21 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 46:21 note "in call to 'bad(0)'"
//@ diag: 47:30 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 47:30 note "in call to 'bad(0)'"
//@ diag: 48:21 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 48:21 note "in call to 'bad(0)'"
//@ diag: 49:33 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 49:40 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 42:38 note "division by zero"
//@ diag: 49:40 note "in call to 'bad(0)'"
//@ diag: 50:35 warning "overflow in expression; result is -2'147'483'648 with type 'int'"
//@ diag: 51:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 52:21 note "in the default initializer of 'x'"
//@ diag: 51:9 note "declared here"
//@ diag: 42:38 note "division by zero"
//@ diag: 51:17 note "in call to 'bad(0)'"
//@ diag: 51:17 error "call to consteval function 'bad' is not a constant expression"
//@ diag: 52:29 note "in the default initializer of 'x'"
//@ diag: 51:9 note "declared here"
//@ diag: 42:38 note "division by zero"
//@ diag: 51:17 note "in call to 'bad(0)'"
consteval int bad(int x) { return 10 / x; }
consteval int one() { return 1; }
int ovf(int x) { return x + bad(0); }
char conv() { return bad(0); }
int cond() { return bad(0) ? 1 : 2; }
bool cmp(int x) { return x < bad(0); }
int neg() { return -bad(0); }
int warn() { return (2147483647 + 1) + bad(0); }
int wrapped() { return 2147483647 + one(); }
int def(int x = bad(0)) { return x; }
int uses() { return def() + def(); }
