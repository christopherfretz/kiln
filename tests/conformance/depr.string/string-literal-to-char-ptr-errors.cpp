// [depr.string]: -pedantic-errors makes the C++11 extension an error; only an
// ordinary or wide literal itself (parenthesized at most) converts, never a
// `u`/`U` literal, a conditional expression or a `const char*` (WP-1467).
//@ std: c++17
//@ args: -pedantic-errors
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:11 error "ISO C++11 does not allow conversion from string literal to 'char *'"
//@ diag: 13:11 error "cannot initialize a variable of type 'char16_t *' with an lvalue of type 'const char16_t[4]'"
//@ diag: 14:7 error "cannot initialize a variable of type 'char *' with an lvalue of type 'const char[2]'"
//@ diag: 16:7 error "cannot initialize a variable of type 'char *' with an lvalue of type 'const char *'"
char *p = "abc";
char16_t *u = u"abc";
char *c = true ? "a" : "b";
const char *s = "x";
char *t = s;
