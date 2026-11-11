// [lex.string.uneval]/1, [lex.ccon]/7-8: each numeric, conditional or
// unknown escape in an unevaluated string literal (a static_assert message)
// is an error naming the whole escape sequence (clang `ProcessCharEscape`:
// the text from the backslash to the end of the escape, after the escape's
// own diagnostic). Then the declaration is dropped. Two cases keep it, as in
// clang: `\x` with no hex digits (an error that does not invalidate the
// string; its value is 0, printed `<U+0000>`) and the simple escapes and
// UCNs, which are valid.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 39:24 error "invalid escape sequence '\\101' in an unevaluated string literal"
//@ diag: 40:24 error "invalid escape sequence '\\0' in an unevaluated string literal"
//@ diag: 41:24 error "octal escape sequence out of range"
//@ diag: 41:24 error "invalid escape sequence '\\777' in an unevaluated string literal"
//@ diag: 42:24 error "invalid escape sequence '\\o{101}' in an unevaluated string literal"
//@ diag: 43:24 error "expected '{' after '\\o' escape sequence"
//@ diag: 43:24 error "invalid escape sequence '\\o' in an unevaluated string literal"
//@ diag: 44:24 error "invalid digit '9' in escape sequence"
//@ diag: 44:24 error "invalid escape sequence '\\o{19}' in an unevaluated string literal"
//@ diag: 45:24 error "invalid escape sequence '\\x01b' in an unevaluated string literal"
//@ diag: 46:24 error "invalid escape sequence '\\x{41}' in an unevaluated string literal"
//@ diag: 47:24 error "invalid digit 'g' in escape sequence"
//@ diag: 47:24 error "invalid escape sequence '\\x{4g}' in an unevaluated string literal"
//@ diag: 48:24 error "hex escape sequence out of range"
//@ diag: 48:24 error "invalid escape sequence '\\x{FFFFFFFFF}' in an unevaluated string literal"
//@ diag: 49:24 error "\\x used with no following hex digits"
//@ diag: 49:15 error "static assertion failed: a<U+0000>g"
//@ diag: 50:24 error "\\x used with no following hex digits"
//@ diag: 50:15 error "static assertion failed: a<U+0000>"
//@ diag: 51:24 warning "unknown escape sequence '\\q' [-Wunknown-escape-sequence]"
//@ diag: 51:24 error "invalid escape sequence '\\q' in an unevaluated string literal"
//@ diag: 52:24 error "invalid escape sequence '\\e' in an unevaluated string literal"
//@ diag: 53:24 error "invalid escape sequence '\\(' in an unevaluated string literal"
//@ diag: 54:24 error "incomplete universal character name"
//@ diag: 55:24 error "expected '}'"
//@ diag: 56:15 error "static assertion failed: aAbBC"
//@ diag: 57:15 error "static assertion failed: '\"?\\<U+0007><U+0008>"
static_assert(false, "a\101b");
static_assert(false, "a\0b");
static_assert(false, "a\777b");
static_assert(false, "a\o{101}b");
static_assert(false, "a\og");
static_assert(false, "a\o{19}b");
static_assert(false, "a\x01b");
static_assert(false, "a\x{41}b");
static_assert(false, "a\x{4g}b");
static_assert(false, "a\x{FFFFFFFFF}b");
static_assert(false, "a\xg");
static_assert(false, "a\x");
static_assert(false, "a\qb");
static_assert(false, "a\eb");
static_assert(false, "a\(b");
static_assert(false, "a\u00");
static_assert(false, "a\u{41");
static_assert(false, "a\U00000041b\u{42}\N{LATIN CAPITAL LETTER C}");
static_assert(false, "\'\"\?\\\a\b");
static_assert(true, "\f\n\r\t\v");
