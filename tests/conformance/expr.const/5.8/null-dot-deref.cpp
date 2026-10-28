// [expr.unary.op]/1: `(*p).m` with p null reads through the null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_dot_deref`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:32 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 10:15 note "in call to 'f(nullptr)'"
struct S { int a; int b; };
constexpr int f(S *p) { return (*p).b; }
static_assert(f(nullptr) == 0);
