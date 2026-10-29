// [new.delete.placement]: without <new>, non-allocating placement new has no operator.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:19 error "no matching 'operator new' function for non-allocating placement new expression; include <new>"
void f(void *p) { new (p) int; }
