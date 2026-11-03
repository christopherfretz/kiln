// [expr.call]/2, [expr.ref]/7.3.2, [expr.const]/5: `__builtin_classify_type`
// takes exactly one argument, cannot be named other than as a callee, and
// its argument is checked as any call argument: a bound member function
// must be called and a builtin must be called directly (clang
// `CheckBuiltinFunctionCall` with `checkArgCount`). spec/sema.md §5.4
// (WP-419).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:33 error "too few arguments to function call, expected 1, have 0"
//@ diag: 16:36 error "too many arguments to function call, expected 1, have 2"
//@ diag: 17:11 error "builtin functions must be directly called"
//@ diag: 18:37 error "reference to non-static member function must be called; did you mean to call it with no arguments?"
//@ diag: 19:33 error "builtin functions must be directly called"
struct S { void m(); };
int a = __builtin_classify_type();
int b = __builtin_classify_type(1, 2.0);
auto p = &__builtin_classify_type;
int c = __builtin_classify_type(S().m);
int d = __builtin_classify_type(__builtin_abs);
