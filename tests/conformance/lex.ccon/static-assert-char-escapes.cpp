// [lex.ccon], [dcl.pre]/12: a character literal in a failed static
// assertion's printed requirement is spelled as clang's
// CharacterLiteral::print does: \x00 for a null character, \xhh for a byte
// that is not printable ASCII (octal escapes included), \uhhhh / \Uhhhhhhhh
// for wider code units, the simple escapes for \' \\ \n and friends.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:15 error "static assertion failed due to requirement 'f('\\x00') == 1'"
//@ diag: 20:23 note "expression evaluates to '0 == 1'"
//@ diag: 21:15 error "static assertion failed due to requirement 'f('\\xc8') + f('\\'') == 1'"
//@ diag: 21:35 note "expression evaluates to '239 == 1'"
//@ diag: 22:15 error "static assertion failed due to requirement 'f('\\\\') + f('\\n') + f('A') == 1'"
//@ diag: 22:45 note "expression evaluates to '167 == 1'"
//@ diag: 23:15 error "static assertion failed due to requirement 'g(L'\\u1234') == 1'"
//@ diag: 23:28 note "expression evaluates to '4660 == 1'"
//@ diag: 24:15 error "static assertion failed due to requirement 'g(U'\\U0001f600') + g(u8'\\x7f') == 1'"
//@ diag: 24:46 note "expression evaluates to '128639 == 1'"
constexpr int f(unsigned char c) { return c; }
constexpr long g(char32_t c) { return c; }
static_assert(f('\0') == 1);
static_assert(f('\310') + f('\'') == 1);
static_assert(f('\\') + f('\n') + f('\101') == 1);
static_assert(g(L'\x1234') == 1);
static_assert(g(U'\U0001F600') + g(u8'\x7f') == 1);
