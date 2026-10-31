// [stmt.ranged]: before C++20 an init-statement in a range-based for is an
// extension (clang `-Wc++20-extensions`, on by default).
//@ std: c++17
//@ diag: 9:8 warning "range-based for loop initialization statements are a C++20 extension"
//@ strict-diags

int f() {
  int s = 0;
  for (int a[] = {1, 2}; int x : a) s += x;
  return s;
}
