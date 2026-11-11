// C11 6.4.2.1p3 and Annex D: D.1 lists the characters an identifier may
// contain and D.2 those it may not start with; C11 is wider than C99
// (U+00A8, emoji) but rejects U+00D7. A combining mark (D.2) cannot
// start an identifier: clang drops it there with an error.
//@ std: c11
//@ args: -pedantic
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "character <U+0300> not allowed at the start of an identifier"
//@ diag: 13:6 error "character <U+00D7> not allowed in an identifier"
//@ diag: 14:12 error "character <U+00D7> not allowed in an identifier"
int ̀y;
int a×;
int \u00A8b\u00D7;
int ¨a = 1, \U0001F600 = 2, q̀;
int *p = &\u00A8a, *e = &😀, *r = &q\u0300, *s = &y;
