// [expr.ref]/2: -> needs a pointer to class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:22 error "member reference type 'S' is not a pointer; did you mean to use '.'?"
struct S { int a; };
int f(S s) { return s->a; }
