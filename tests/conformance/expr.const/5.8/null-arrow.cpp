// [expr.ref]/1: member access through a null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_arrow`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:35 note "cannot access field of null pointer"
//@ diag: 10:15 note "in call to 'f(nullptr)'"
struct S { int a; };
constexpr int f(S *p) { return p->a; }
static_assert(f(nullptr) == 0);
