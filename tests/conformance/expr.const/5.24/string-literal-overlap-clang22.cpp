// [lex.string]/9, [expr.const]/5.24 (clang 22 `ArePotentiallyOverlappingStringLiterals`):
// two string literal objects may share storage only where their bytes
// agree, so pointers into them compare as constants unless, after the
// offset shift, the shorter literal's bytes could be a prefix of the other
// (or both are the same evaluation). A pointer into a literal and a
// pointer to another object always compare.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:21 note "comparison of addresses of potentially overlapping literals has unspecified value"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 23:21 note "comparison of addresses of potentially overlapping literals has unspecified value"
constexpr char x = 0;
constexpr const char *p = "abc";
constexpr const char *q = "abc";
constexpr const char *r = "bc";
static_assert(p + 1 != r);
static_assert(p != &x);
static_assert(p + 1 != q);
static_assert(p != r);
static_assert(+"abc" != +"bc");
static_assert(p + 3 != r + 2);
