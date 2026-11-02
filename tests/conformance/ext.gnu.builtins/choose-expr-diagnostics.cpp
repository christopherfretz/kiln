// __builtin_choose_expr's condition must be an integral constant
// expression: "'__builtin_choose_expr' requires a constant expression"
// at the condition with the evaluation's notes (also on substitution),
// and the three operands are comma-separated (clang 19 ActOnChooseExpr,
// ParseBuiltinPrimaryExpression; WP-307).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 22:31 error "'__builtin_choose_expr' requires a constant expression"
//@ diag: 22:31 note "read of non-const variable 'x' is not allowed in a constant expression"
//@ diag: 20:5 note "declared here"
//@ diag: 23:31 error "'__builtin_choose_expr' requires a constant expression"
//@ diag: 24:31 error "'__builtin_choose_expr' requires a constant expression"
//@ diag: 27:35 error "expected ','"
//@ diag: 28:34 error "use of undeclared identifier 'nope'"
//@ diag: 25:61 error "'__builtin_choose_expr' requires a constant expression"
//@ diag: 26:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 25:61 note "function parameter 't' with unknown value cannot be used in a constant expression"
//@ diag: 25:27 note "declared here"
int x = 3;
double d = 1;
int a = __builtin_choose_expr(x, 1, 2);
int b = __builtin_choose_expr(d, 1, 2);
int c = __builtin_choose_expr(1.5, 1, 2);
template<class T> int f(T t) { return __builtin_choose_expr(t, 1, 2); }
int g = f(1);
int h = __builtin_choose_expr(1, 2);
int i = __builtin_choose_expr(1, nope, 2);
