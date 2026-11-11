// C23 6.7.10p2 (initialization): no initializer shall provide a value for
// an object outside the entity; a too-long string literal for a char
// array is truncated with a warning in C (-Wexcess-initializers), an error
// in C++ ([dcl.init.string]p2). The exact-fit case is silent (p15).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:13 warning "initializer-string for char array is too long"
//@ diag: 12:15 warning "initializer-string for char array is too long"
char a[3] = "abc";
char b[2] = "abc";
void f(void) {
  char l[1] = "xyz";
  (void)l;
}
