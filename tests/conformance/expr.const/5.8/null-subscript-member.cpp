// [expr.add]/4.1: `p[0]` on null is allowed, the member read is not.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_subscript_member`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:32 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 10:15 note "in call to 'f(nullptr)'"
struct S { int a; int b; };
constexpr int f(S *p) { return p[0].b; }
static_assert(f(nullptr) == 0);
