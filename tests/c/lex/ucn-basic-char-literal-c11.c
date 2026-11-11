// C11 6.4.3p2: a universal character name shall not name a basic source
// character, even in a literal (C23 and C++11 lift that for literals).
//@ std: c11
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:18 error "character 'A' cannot be specified by a universal character name"
//@ diag: 9:10 error "character '%' cannot be specified by a universal character name"
const char *e = "\u0041";
int c = '\u0025';
const char *ok = "\u0024\u00C0";
