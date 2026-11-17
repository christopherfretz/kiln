// [stmt.pre]/4 (C++26, P0963R3): before C++17 a structured binding
// declaration is accepted as an extension; its warning ("structured
// binding declarations are a C++17 extension") replaces the one for a
// condition, as in clang. An array initializer is still rejected.
// Found running GCC's C++ testsuite.
//@ std: c++14
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 20:8 warning "structured binding declarations are a C++17 extension"
//@ diag: 21:12 warning "structured binding declarations are a C++17 extension"
//@ diag: 22:12 warning "structured binding declarations are a C++17 extension"
//@ diag: 22:12 error "an array type is not allowed here"
struct P {
  int x, y;
  explicit operator bool() const { return x != 0; }
};
int arr[2] = {1, 2};
int f(P p) {
  auto [u, v] = p;
  if (auto [a, b] = p) return a + u + v;
  if (auto [a, b] = arr) return a;
  return 0;
}
