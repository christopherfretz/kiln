// [expr.unary.op]/1, [expr.const]/5.8: indirection through a null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_deref`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:34 note "null pointer is not allowed in a constant expression"
//@ diag: 11:15 note "in call to 'f(nullptr)'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
constexpr int f(int *p) { return *p; }
static_assert(f(nullptr) == 0);
