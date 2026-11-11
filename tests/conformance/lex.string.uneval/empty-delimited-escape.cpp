// [lex.ccon]/8, [lex.string.uneval]/1 (P2290 delimited escapes): an empty
// `\x{}` or `\o{}` is an error, and clang 22 consumes its `}`: in an
// unevaluated string it is then an invalid escape and the static_assert is
// dropped; in a character literal (also in `#if`) the escape is the whole
// literal. clang 19 leaves the `}` (see empty-delimited-escape-19.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 15:24 error "delimited escape sequence cannot be empty"
//@ diag: 15:24 error "invalid escape sequence '\\x{}' in an unevaluated string literal"
//@ diag: 16:24 error "delimited escape sequence cannot be empty"
//@ diag: 16:24 error "invalid escape sequence '\\o{}' in an unevaluated string literal"
//@ diag: 17:10 error "delimited escape sequence cannot be empty"
//@ diag: 18:6 error "delimited escape sequence cannot be empty"
static_assert(false, "a\x{}b");
static_assert(false, "a\o{}b");
int c = '\x{}';
#if '\x{}'
#endif
