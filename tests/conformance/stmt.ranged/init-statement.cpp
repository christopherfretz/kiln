// [stmt.ranged]/1 (C++20, P0614): an init-statement before the
// for-range-declaration; its declarations are in scope in the range and
// the body.
//@ expect: run
//@ std: c++20
//@ stdout: 12 3
#include <initializer_list>
extern "C" int printf(const char*, ...);

int main() {
  int s = 0;
  for (int a[] = {3, 4, 5}; int x : a) s += x;
  int n = 0;
  for (n = 1; int x : {1, 2}) n += x - 1;
  printf("%d %d\n", s, n + 1);
}
