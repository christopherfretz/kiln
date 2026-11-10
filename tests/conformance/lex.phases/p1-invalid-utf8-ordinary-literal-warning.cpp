// [lex.phases]/1.1, [lex.string], [lex.ccon]: source bytes that are not valid UTF-8
// inside an ordinary string or character literal are accepted with clang's
// -Winvalid-source-encoding warning (on by default) and kept as they are. A string
// literal gets one warning per fragment between escapes, at its first invalid
// byte; a character literal gets one at its start, and each byte is a c-char.
// The bytes here are Latin-1 (0xE9 is e-acute) and truncated UTF-8.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:21 warning "illegal character encoding in string literal"
//@ diag: 23:18 warning "illegal character encoding in string literal"
//@ diag: 24:18 warning "illegal character encoding in string literal"
//@ diag: 24:21 warning "illegal character encoding in string literal"
//@ diag: 25:23 warning "illegal character encoding in string literal"
//@ diag: 26:10 warning "illegal character encoding in character literal"
//@ diag: 27:19 warning "illegal character encoding in character literal"
//@ diag: 27:19 warning "multi-character character constant"
//@ diag: 29:17 warning "illegal character encoding in string literal"
//@ diag: 28:18 note "expanded from macro 'LATIN1'"
//@ diag: 30:5 warning "illegal character encoding in character literal"
//@ diag: 32:26 warning "illegal character encoding in string literal"
//@ diag: 34:43 warning "illegal character encoding in string literal"
const char *a = "café";
const char *b = "éé Ã xðŸ";
const char *c = "é\né";
const char *d = R"(café)";
char e = 'é';
constexpr int f = 'aé';
#define LATIN1 "déjà"
const char *g = LATIN1;
#if 'é' != 0
#endif
static_assert(sizeof("café") == 5);
static_assert(f == 0x61E9);
static_assert(static_cast<unsigned char>("é"[0]) == 0xE9);
/* é in a comment is not diagnosed */ // nor here: é
#if 0
int é skipped;
#endif
