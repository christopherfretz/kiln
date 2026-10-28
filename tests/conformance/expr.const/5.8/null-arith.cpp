// [expr.add]/4.1: only 0 may be added to a null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_arith`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:37 note "cannot perform pointer arithmetic on null pointer"
//@ diag: 9:15 note "in call to 'f(nullptr)'"
constexpr bool f(int *p) { return p + 1 != nullptr; }
static_assert(f(nullptr));
