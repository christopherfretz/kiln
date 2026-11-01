// [lex.charset]/3, [lex.universal.char] (C++23, P2071R2): named universal
// character escapes `\N{...}`. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
static_assert(U'\N{LATIN CAPITAL LETTER A}' == U'A');
static_assert(U'\N{GREEK SMALL LETTER ALPHA}' == U'α');
static_assert(u8"\N{DIGIT ONE}"[0] == u8'1');
