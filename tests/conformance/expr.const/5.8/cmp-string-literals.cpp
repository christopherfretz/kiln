// [lex.string]/9, [expr.const]/5.24: distinct literal evaluations need not be distinct objects.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_cmp_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:20 note "literals has unspecified value"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
static_assert(+"a" == "a");
