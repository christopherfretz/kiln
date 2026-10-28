//@ expect: ast
consteval int sq(int x) { return x * x; }
constexpr int cube(int x) { return x * sq(2) * x; }
int run(int y) {
  int a = sq(5);
  return a + cube(y);
}
