// P2558R2 ([lex.charset]): `@`, `$` and the grave accent are in the basic
// character set; they are preprocessing tokens that can be stringized
// and appear in character and string literals.
//@ expect: compile-ok
//@ std: c++26
//@ strict-diags
#define S(x) #x
static_assert(sizeof(S(@)) == 2);
static_assert(sizeof(S($)) == 2);
static_assert(sizeof(S(`)) == 2);
static_assert(u8"@$`"[0] == '@' && u8"@$`"[2] == '`');
constexpr char c = '`';
static_assert(c == 0x60);
