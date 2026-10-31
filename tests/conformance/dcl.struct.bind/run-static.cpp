//@ expect: run
//@ stdout: 1 2 3 4 5 6 3
// [dcl.pre]/6 (C++20 P1091) at run time: namespace scope and static local
// structured binding declarations with constant initializers; the holding
// variable is a mangled global (`_ZDC...E`) and a static local keeps its
// state across calls.
extern "C" int printf(const char *, ...);
struct P { int x, y; };
constexpr P cp{1, 2};
auto [gx, gy] = cp;
namespace N {
int arr[2] = {3, 4};
auto &[n0, n1] = arr;
}
int counter() {
  static auto [calls, unused] = P{0, 0};
  return ++calls;
}
int main() {
  N::n1 += 0;
  counter();
  counter();
  static auto [sx, sy] = P{5, 6};
  printf("%d %d %d %d %d %d %d\n", gx, gy, N::n0, N::n1, sx, sy, counter());
}
