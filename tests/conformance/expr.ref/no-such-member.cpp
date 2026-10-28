// [expr.ref]/6: the member must be found in the class
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:23 error "no member named 'b' in 'S'"
struct S { int a; };
int f(S s) { return s.b; }
