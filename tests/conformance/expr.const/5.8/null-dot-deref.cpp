// [expr.unary.op]/1: `(*p).m` with p null reads through the null pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_dot_deref`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11 note "null pointer is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f(nullptr)'"
// clang 19 and 22 word and place these null-pointer notes differently; kiln
// follows its -fclang-compat level (WP-377), so the directives are column-less.
struct S { int a; int b; };
constexpr int f(S *p) { return (*p).b; }
static_assert(f(nullptr) == 0);
