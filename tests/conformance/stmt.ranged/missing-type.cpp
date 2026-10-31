// [stmt.ranged]: `for (x : range)` (the abandoned N3994 terse form) has no
// type for the loop variable.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:8 error "range-based for loop requires type for loop variable"
int a[3];
void f() {
  for (x : a) {}
}
