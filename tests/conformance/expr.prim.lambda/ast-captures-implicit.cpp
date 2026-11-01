//@ expect: ast
//@ ast-implicit
//@ ast-filter: lamtest
// [expr.prim.lambda.closure]/10-12, [expr.prim.lambda.capture]/10-12: a
// capturing closure has an unnamed member per capture (a reference member
// for a by-reference capture), a deleted default constructor and copy
// assignment, no move assignment, and a destructor.
struct N { int v; N(int); N(const N&); ~N(); };
int lamtest(int a, N n) {
  auto c = [a, &n] { return a + n.v; };
  auto d = [n, k = a * 2]() mutable { return n.v + k; };
  return c() + d();
}
