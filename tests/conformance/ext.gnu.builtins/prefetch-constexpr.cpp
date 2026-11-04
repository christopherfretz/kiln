// A __builtin_prefetch call is not a constant expression: clang 19 and 22
// (`ExprConstant` evaluates no `BI__builtin_prefetch`) and GCC reject it
// inside a constant evaluation, [expr.const]/10. WP-610.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:28 note "subexpression not valid in a constant expression"
//@ diag: 11:15 note "in call to 'g()'"
//@ diag: 12:20 error "constexpr variable 'n' must be initialized by a constant expression"
constexpr int g() { int x; __builtin_prefetch(&x); return 1; }
static_assert(g() == 1);
constexpr int n = (__builtin_prefetch(nullptr), 1);
int h() { int x = 0; __builtin_prefetch(&x, 1, 2); return x; }
