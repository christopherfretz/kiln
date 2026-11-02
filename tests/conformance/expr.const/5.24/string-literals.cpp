// [expr.const]/5.24, [lex.string]/9: two literal evaluations may or may not share an object.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare -Wno-string-compare
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:33 note "comparison of addresses of literals has unspecified value"
//@ diag: 12:15 note "in call to 'f()'"
//@ requires: clang<22
// clang 22 emits error: comparison between two arrays is ill-formed in C++26; to compare array addr...; WP-355 residual
constexpr bool f() { return "x" != "x"; }
static_assert(f());
