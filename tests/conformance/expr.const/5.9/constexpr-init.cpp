// [expr.const]/5.9, [dcl.constexpr]/6: a constexpr variable's initializer must be a constant expression.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_9_constexpr_init`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "must be initialized by a constant expression"
//@ diag: 9:19 note "read of non-const variable 'g' is not allowed in a constant expression"
//@ diag: 8:5 note "declared here"
int g = 1;
constexpr int x = g + 1;
