// [expr.add]/4.1: `p[0]` on null is allowed, the member read is not.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_subscript_member`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11 note "null pointer"
//@ diag: 12:15 note "in call to 'f(nullptr)'"
// clang 19 and 22 word and place these null-pointer notes differently; kiln
// follows its -fclang-compat level (WP-377), so the directives are column-less.
struct S { int a; int b; };
constexpr int f(S *p) { return p[0].b; }
static_assert(f(nullptr) == 0);
