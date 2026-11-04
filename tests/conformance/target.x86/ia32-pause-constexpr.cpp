// A __builtin_ia32_pause call is not a constant expression
// ([expr.const]/10; clang evaluates no x86 builtin). WP-610.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:21 note "subexpression not valid in a constant expression"
//@ diag: 10:15 note "in call to 'g()'"
constexpr int g() { __builtin_ia32_pause(); return 1; }
static_assert(g() == 1);
void f() { __builtin_ia32_pause(); }
