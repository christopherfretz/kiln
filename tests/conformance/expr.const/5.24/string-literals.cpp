// [expr.const]/5.24, [lex.string]/9: two literal evaluations may or may not share an object.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-deprecated-array-compare -Wno-string-compare
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:33 note "comparison of addresses of literals has unspecified value"
//@ diag: 10:15 note "in call to 'f()'"
constexpr bool f() { return "x" != "x"; }
static_assert(f());
