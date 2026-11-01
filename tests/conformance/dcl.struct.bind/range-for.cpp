//@ expect: ast
//@ ast-filter: test
// [stmt.ranged]/1 with a structured binding declaration as the
// for-range-declaration: the holding variable is the loop variable,
// initialized from `*__begin1`, and the bindings name its members.
struct P { int x, y; };
int test_loop(P (&ps)[2]) {
  int s = 0;
  for (auto [x, y] : ps) s += x * y;
  for (const auto &[x, y] : ps) s += x - y;
  return s;
}
