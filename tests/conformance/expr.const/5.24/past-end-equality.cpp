// [expr.const]/5.24, [expr.eq]/3.1: equality of a past-the-end pointer and a pointer to another object.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_past_end_equality`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:21 note "comparison against pointer '&a[1]' that points past the end of a complete object has unspecified value"
constexpr int a[1] = {1};
constexpr int b = 2;
static_assert(a + 1 != &b);
