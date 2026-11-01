// [dcl.fct] (WP-234): -Wextra enables -Wunused-but-set-parameter.
//@ args: -Wextra
//@ expect: compile-ok
//@ strict-diags
//@ diag: 6:12 warning "parameter 'x' set but not used"
void f(int x) { x = 1; }
void g(int x) { (void)x; }
