// [lex.ccon]/1, [lex.charset]/3 (C++23, P2290R3): delimited escape sequences
// `\o{...}`, `\x{...}` and `\u{...}`. WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
static_assert('\o{101}' == 'A');
static_assert('\x{42}' == 'B');
static_assert(u'\u{43}' == u'C');
static_assert(U'\u{1F600}' == U'\U0001F600');
static_assert(sizeof("\x{41}\o{102}") == 3);
