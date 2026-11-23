// [lex.ext]/1, [lex.name]: a ud-suffix is an identifier, so a
// universal-character-name in it names the character it designates, as
// it does in any other identifier: `1_\u00e9`, `1_\U000000e9`,
// `1_\u{e9}` and `1_\N{LATIN SMALL LETTER E WITH ACUTE}` all call the
// literal operator whose suffix is spelled with that character, and the
// suffix of a literal operator declared with a universal-character-name
// matches the character spelled directly. Reduced from real code.
//@ strict-diags
//@ std: c++26
using Z = decltype(sizeof 0);
constexpr int operator""_é(unsigned long long v) { return int(v); }
constexpr int operator""_\u00e8(unsigned long long v) { return int(v) + 100; }
constexpr Z operator""_\u00e9(const char *, Z n) { return n; }
constexpr char operator""_é(char c) { return c; }
static_assert(1_\u00e9 == 1);
static_assert(2_\U000000e9 == 2);
static_assert(3_\u{e9} == 3);
static_assert(4_\N{LATIN SMALL LETTER E WITH ACUTE} == 4);
static_assert(5_é == 5);
static_assert(6_è == 106);
static_assert(7_\u00e8 == 107);
static_assert("ab"_é == 2);
static_assert('c'_\u00e9 == 'c');
