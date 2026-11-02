// [stmt.pre]/1 (C++23, P2360R0): before C++23 an alias-declaration is not
// an init-statement; clang accepts it in a range-based `for` as a C++23
// extension.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ diag: 10:8 warning "alias declaration in this context is a C++23 extension"
int f() {
  int s = 0, arr[] = {1, 2};
  for (using T = int; T i : arr) s += i;
  return s;
}
