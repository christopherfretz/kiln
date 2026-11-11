// C has delimited and named escape sequences only as an extension:
// clang 19 calls both a Clang extension at the escape (C++: a C++23
// extension; [lex.ccon], [lex.charset]). WP-1312, WP-1299 follow-up.
//@ requires: clang<22
//@ std: c17
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 15:11 warning "named escape sequences are a Clang extension"
//@ diag: 16:18 warning "delimited escape sequences are a Clang extension"
//@ diag: 16:24 warning "named escape sequences are a Clang extension"
//@ diag: 17:5 warning "named escape sequences are a Clang extension"
//@ diag: 18:10 warning "delimited escape sequences are a Clang extension"
//@ diag: 19:5 warning "delimited escape sequences are a Clang extension"
int a = L'\N{GREEK SMALL LETTER BETA}';
const char *s = "\x{41}\N{GREEK SMALL LETTER ALPHA}";
int \N{GREEK SMALL LETTER ALPHA} = 1;
int b = '\o{101}';
int \u{3b2} = 2;
