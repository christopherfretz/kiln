// C23 6.2.7p2, 6.7p4 (declarations): a block-scope function or `extern`
// object declaration and a later file-scope declaration of the same name
// refer to one entity, so their types must be compatible.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:5 error "conflicting types for 'foo'"
//@ diag: 10:21 note "previous declaration is here"
//@ diag: 12:6 error "redefinition of 'v' with a different type: 'long' vs 'int'"
//@ diag: 10:42 note "previous declaration is here"
void a(void) { long foo(int); extern int v; }
int foo(void);
long v;
