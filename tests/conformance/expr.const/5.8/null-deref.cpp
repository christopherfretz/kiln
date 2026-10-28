// [expr.unary.op]/1, [expr.const]/5.8: indirection through a null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_deref`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:34 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 9:15 note "in call to 'f(nullptr)'"
constexpr int f(int *p) { return *p; }
static_assert(f(nullptr) == 0);
