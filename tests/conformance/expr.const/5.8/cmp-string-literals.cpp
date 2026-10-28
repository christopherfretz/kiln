// [lex.string]/9, [expr.const]/5.24: distinct literal evaluations need not be distinct objects.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare
//@ diag: 8:15 error "static assertion expression is not an integral constant expression"
//@ diag: 8:19 note "comparison of addresses of literals has unspecified value"
static_assert("a" == "a");
