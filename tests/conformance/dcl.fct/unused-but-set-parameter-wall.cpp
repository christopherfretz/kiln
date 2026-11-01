// [dcl.fct] (WP-234): neither -Wall nor -Wunused-but-set-variable enables
// -Wunused-but-set-parameter.
//@ args: -Wall -Wunused-but-set-variable
//@ expect: compile-ok
//@ strict-diags
void f(int x) { x = 1; }
