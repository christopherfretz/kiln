// [expr.delete]: delete of a pointer to array is treated as delete[] (clang warning).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 6:23 warning "'delete' applied to a pointer-to-array type 'int (*)[4]' treated as 'delete[]'"
//@ diag: 8:19 error "cannot initialize return object of type 'int *' with an lvalue of type 'int'"
void f(int (*p)[4]) { delete p; }
int g;
int *h() { return g; }
