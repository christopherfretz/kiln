// C23 6.7.11/4 (WP-1739): before C23 the empty initializer of a variable
// length array is accepted as clang accepts it, with its C23-extension
// warning under -pedantic; a non-empty list stays "variable-sized object
// may not be initialized" (vla-folded-to-constant-neg.c).
//@ std: c17
//@ args: -pedantic
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:28 warning "use of an empty initializer is a C23 extension"
void g(int *);
void f(int n) { int a[n] = {}; g(a); }
