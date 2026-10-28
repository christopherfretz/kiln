// [lex.string]/9: a pointer frozen into a `constexpr` variable keeps the
// identity of the literal evaluation that created it, so pointers into
// two equal literals compare unspecified (not equal through a shared
// object), and a pointer compares equal to itself.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_frozen_string_literals`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:21 note "comparison of addresses of literals has unspecified value"
constexpr const char *p = "abc";
constexpr const char *q = "abc";
static_assert(p + 1 != q);
