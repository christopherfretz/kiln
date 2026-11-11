// C has delimited and named escape sequences only as an extension
// (clang 22 twin of delimited-escape-extension-19.c): clang 22 calls a
// named one a Clang extension and a delimited one a C2y extension, and
// reports those in a literal at the literal's start (C++: C++23
// extensions; [lex.ccon], [lex.charset]). WP-1312, WP-1299 follow-up.
//@ requires: clang>=22
//@ std: c17
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:9 warning "named escape sequences are a Clang extension"
//@ diag: 18:17 warning "delimited escape sequences are a C2y extension"
//@ diag: 18:17 warning "named escape sequences are a Clang extension"
//@ diag: 19:5 warning "named escape sequences are a Clang extension"
//@ diag: 20:9 warning "delimited escape sequences are a C2y extension"
//@ diag: 21:5 warning "delimited escape sequences are a C2y extension"
int a = L'\N{GREEK SMALL LETTER BETA}';
const char *s = "\x{41}\N{GREEK SMALL LETTER ALPHA}";
int \N{GREEK SMALL LETTER ALPHA} = 1;
int b = '\o{101}';
int \u{3b2} = 2;
