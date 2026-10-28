// [expr.const]/5.4: an invocation of a constexpr function that is declared but not defined.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_4_undefined_function`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:28 note "undefined function 'u' cannot be used in a constant expression"
//@ diag: 11:15 note "in call to 'f()'"
//@ diag: 9:15 note "declared here"
constexpr int u();
constexpr int f() { return u(); }
static_assert(f() == 1);
