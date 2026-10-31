//@ expect: ast
//@ ast-filter: test
// [dcl.pre]/6 (C++20 P1091): a structured binding declaration may be static
// or thread_local, and may appear at namespace scope; the holding variable
// then has static storage duration.
struct P { int x, y; };
P gp{1, 2};
namespace test_ns {
auto [gx, gy] = gp;
}
int test_static() {
  static auto [sx, sy] = gp;
  return sx + sy + test_ns::gx;
}
