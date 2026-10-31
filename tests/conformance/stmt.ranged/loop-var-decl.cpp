// [stmt.ranged], [dcl.stc]: the for-range-declaration may not have a
// storage class or define a type; the loop variable is initialized from
// `*__begin` ([dcl.init]), so a mismatched type, a reference dropping
// `const` and assignment to a `const` variable are diagnosed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:8 error "loop variable 'x' may not be declared 'static'"
//@ diag: 20:8 error "loop variable 'y' may not be declared 'extern'"
//@ diag: 21:8 error "types may not be defined in a for range declaration"
//@ diag: 22:13 error "cannot initialize a variable of type 'int *' with an lvalue of type 'int'"
//@ diag: 23:13 error "binding reference of type 'int' to value of type 'const int' drops 'const' qualifier"
//@ diag: 24:29 error "cannot assign to variable 'x' with const-qualified type 'const int'"
//@ diag: 24:18 note "variable 'x' declared const here"
#include <initializer_list>
int a[3];
struct R { int* begin(); int* end(); };

void f() {
  for (static int x : a) {}
  for (extern int y : a) {}
  for (struct T { int v; } t : a) {}
  for (int* q : a) {}
  for (int& x : {1, 2}) {}
  for (const int x : R{}) x = 1;
}
