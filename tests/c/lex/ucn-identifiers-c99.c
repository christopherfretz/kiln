// C99 6.4.2.1p3 and Annex D: an identifier's universal character names
// and UTF-8 characters come from Annex D's ranges, and Annex D's digits
// cannot start one. clang drops a UTF-8 character that cannot start an
// identifier (with an error) and keeps one that cannot continue it.
//@ std: c99
//@ args: -pedantic
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:5 error "character <U+0660> not allowed at the start of an identifier"
//@ diag: 13:6 error "character <U+00A8> not allowed in an identifier"
//@ diag: 14:6 error "character <U+1F600> not allowed in an identifier"
int ٠y;
int a¨;
int z😀;
int \u00C0a = 1, \u00AAα = 2, c_\u0660١;
int *p = &Àa, *q = &ª\u03B1, *r = &c_٠\u0661;
int *s = &y;
