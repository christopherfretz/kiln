// [dcl.constexpr]/3 (C++20, P1331R2): trivial default initialization in
// constexpr functions; the variable must be assigned before it is read.
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
constexpr int f(int n) {
  int x;
  int arr[4];
  for (int i = 0; i < 4; ++i) arr[i] = i * n;
  x = arr[3];
  return x;
}
struct T { int a; };
constexpr int g() { T t; t.a = 5; return t.a; }
static_assert(f(2) == 6 && g() == 5);
