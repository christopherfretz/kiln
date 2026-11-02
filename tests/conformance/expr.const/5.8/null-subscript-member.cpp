// [expr.add]/4.1: `p[0]` on null is allowed, the member read is not.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_subscript_member`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:32 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f(nullptr)'"
//@ requires: clang<22
// clang 22 emits error: static assertion expression is not an integral constant expression; WP-355 residual
struct S { int a; int b; };
constexpr int f(S *p) { return p[0].b; }
static_assert(f(nullptr) == 0);
