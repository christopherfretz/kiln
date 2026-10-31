//@ expect: ast
// [expr.const]/16: immediate invocations of class type fold to a
// ConstantExpr; only the outermost of nested invocations is wrapped,
// and a constant-initialized variable keeps its invocations unevaluated.
struct P { int a; int b; };
consteval P mk(int x) { return {x, x + 1}; }
consteval int sum(P p) { return p.a + p.b; }
constexpr int k = sum(mk(2));
int run() {
  P p = mk(3);
  int s = sum(mk(4));
  int t = mk(5).b;
  return p.a + s + t + k;
}
