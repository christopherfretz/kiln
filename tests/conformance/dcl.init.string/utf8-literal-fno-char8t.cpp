// [dcl.init.string]/1 under `-fno-char8_t` (clang `Char8` off): a `u8`
// string literal is a `const char[N]`, so it initializes any narrow
// character array, `signed char` included, as an ordinary literal does.
// spec/sema.md "String literal initializers", WP-697 (WP-823).
//@ expect: compile-ok
//@ args: -fno-char8_t
//@ strict-diags
signed char a[] = u8"x";
unsigned char b[] = u8"xy";
char c[] = {u8"x"};
static_assert(sizeof(b) == 3);
