// [expr.unary.op]/1, [expr.const]/5.8: indirection through a null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_deref`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:34 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 11:15 note "in call to 'f(nullptr)'"
//@ requires: clang<22
// clang 22 emits error: static assertion expression is not an integral constant expression; WP-355 residual
constexpr int f(int *p) { return *p; }
static_assert(f(nullptr) == 0);
