// [dcl.fct] (WP-682): a bare `-W` is clang's old spelling of `-Wextra`
// (googletest's strict flags pass it), so it enables
// -Wunused-but-set-parameter like -Wextra does.
//@ args: -W
//@ expect: compile-ok
//@ strict-diags
//@ diag: 8:12 warning "parameter 'x' set but not used"
void f(int x) { x = 1; }
void g(int x) { (void)x; }
