// [expr.const]/5.24, [lex.string]/9: two literal evaluations may or may not share an object.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare -Wno-string-compare
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:34 note "literals has unspecified value"
//@ diag: 12:15 note "in call to 'f()'"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
constexpr bool f() { return +"x" != "x"; }
static_assert(f());
