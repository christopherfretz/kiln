// [expr.ref]/2: built-in `->` needs a pointer to a complete class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:28 error "member reference base type 'int' is not a structure or union"
//@ diag: 11:30 error "member reference base type 'void ()' is not a structure or union"
//@ diag: 12:22 error "member reference type 'int' is not a pointer"
//@ diag: 13:34 error "member reference base type 'int' is not a structure or union"
//@ diag: 15:36 error "member reference type 'E' is not a pointer"
//@ diag: 16:25 error "member reference base type 'int' is not a structure or union"
int *p; int b1() { return p->m; }
void f(); int b2() { return f->m; }
int b3() { return 3->m; }
int arr[3]; int b4() { return arr->m; }
struct S { int m; }; S sa[2]; int b5() { return sa->m; }
enum E { e }; int b6() { return e->m; }
int b7(S s) { return s.m.x; }
