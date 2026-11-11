// C89 has no UCNs (C99 6.4.3 added them): clang lexes `\u00AA` as `\` and an
// identifier (-Wunicode warns, not in a skipped group) and decodes one in a literal.
//@ std: c89
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:11 warning "universal character names are only valid in C99 or C++; treating as"
//@ diag: 12:11 warning "universal character names are only valid in C99 or C++; treating as"
//@ diag: 13:12 warning "universal character names are only valid in C99 or C++; treating as"
//@ diag: 14:19 warning "universal character names are only valid in C99 or C++"
#define D(x) 0
#define M \u00AA
int s = D(\u00AA);
int t = D(a\u00AA);
const char *l = "x\u00AA";
#if 0
\u00AA
#endif
