// [lex.string]/10, [lex.ccon]/8: a \x escape in a UTF-8 literal is a code unit
// value, so u8"c\xff" is well-formed (0xFF fits char8_t, as clang and GCC
// agree) and holds the single code unit 0xFF, not U+00FF encoded as UTF-8.
// A value too large for char8_t is ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:24 error "hex escape sequence out of range"
constexpr const char8_t *s = u8"c\xff";
static_assert(sizeof(u8"c\xff") == 3);
static_assert(s[0] == u8'c' && s[1] == 0xFF && s[2] == 0);
static_assert(u8'\xff' == 0xFF);
static_assert(sizeof(u8"\377") == 2 && u8"\377"[0] == 0xFF);
constexpr const char8_t *x = u8"\u00ff";
static_assert(x[0] == 0xC3 && x[1] == 0xBF);
const char8_t *t = u8"c\x100";
