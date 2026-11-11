// [lex.ccon]/8, [lex.string.uneval]/1 (P2290 delimited escapes), clang 19
// behaviour (kiln -fclang-compat=19): an empty `\x{}` or `\o{}` is an error
// that leaves its `}` unconsumed, so the string keeps "a", U+0000, "}b" and
// the static_assert still fails; the character literal (also in `#if`)
// becomes two characters. clang 22 consumes the `}` (see empty-delimited-escape.cpp).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 17:24 error "delimited escape sequence cannot be empty"
//@ diag: 17:15 error "static assertion failed: a<U+0000>}b"
//@ diag: 18:24 error "delimited escape sequence cannot be empty"
//@ diag: 18:15 error "static assertion failed: a<U+0000>}b"
//@ diag: 19:10 error "delimited escape sequence cannot be empty"
//@ diag: 19:9 warning "multi-character character constant [-Wmultichar]"
//@ diag: 20:6 error "delimited escape sequence cannot be empty"
//@ diag: 20:5 warning "multi-character character constant [-Wmultichar]"
static_assert(false, "a\x{}b");
static_assert(false, "a\o{}b");
int c = '\x{}';
#if '\x{}'
#endif
