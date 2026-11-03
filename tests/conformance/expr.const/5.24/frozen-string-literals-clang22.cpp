// [lex.string]/9, [expr.const]/5.24: clang 22 compares pointers into two
// string literal objects unless the literals could overlap (clang
// `ArePotentiallyOverlappingStringLiterals`): "bc" at `p + 1` and the
// start of "abc" differ in their first character, so they cannot share
// storage and `p + 1 != q` is a constant (clang 19 rejects it).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
// clang 22 spelling of frozen-string-literals.cpp (WP-394).
constexpr const char *p = "abc";
constexpr const char *q = "abc";
static_assert(p + 1 != q);
