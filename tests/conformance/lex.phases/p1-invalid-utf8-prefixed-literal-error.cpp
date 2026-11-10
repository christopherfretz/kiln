// [lex.phases]/1.1, [lex.string], [lex.ccon]: source bytes that are not valid UTF-8
// inside a u8, u, U or L literal are an error, as in clang. An unprefixed piece
// of a prefixed concatenation is an error too. A character literal's error is
// at its start and adds no multi-character error.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:26 error "illegal character encoding in string literal"
//@ diag: 18:26 error "illegal character encoding in string literal"
//@ diag: 19:26 error "illegal character encoding in string literal"
//@ diag: 20:25 error "illegal character encoding in string literal"
//@ diag: 21:23 error "illegal character encoding in string literal"
//@ diag: 21:27 error "illegal character encoding in string literal"
//@ diag: 22:13 error "illegal character encoding in character literal"
//@ diag: 23:14 error "illegal character encoding in character literal"
//@ diag: 24:14 error "illegal character encoding in character literal"
//@ diag: 25:13 error "illegal character encoding in character literal"
const char8_t *a = u8"café";
const char16_t *b = u"café";
const char32_t *c = U"café";
const wchar_t *d = L"café";
const char8_t *e = u8"é" "é";
char8_t f = u8'é';
char16_t g = u'é';
char32_t h = U'é';
wchar_t i = L'aé';
