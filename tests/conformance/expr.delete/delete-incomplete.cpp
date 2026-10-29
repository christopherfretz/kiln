// [expr.delete]/5: deleting a pointer to an incomplete class type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:16 error "cannot delete pointer to incomplete type 'S'"
//@ diag: 6:8 note "forward declaration of 'S'"
struct S;
void f(S *p) { delete p; }
