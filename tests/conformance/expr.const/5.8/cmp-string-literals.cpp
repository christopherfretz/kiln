// [lex.string]/9, [expr.const]/5.24: distinct literal evaluations need not be distinct objects.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:19 note "comparison of addresses of literals has unspecified value"
//@ requires: clang<22
// clang 22 emits error: comparison between two arrays is ill-formed in C++26; to compare array addr...; WP-355 residual
static_assert("a" == "a");
