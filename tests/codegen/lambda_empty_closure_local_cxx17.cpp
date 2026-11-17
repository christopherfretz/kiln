// Before C++20 no closure type has a default constructor (C++17
// [expr.prim.lambda.closure]), so a captureless closure is not trivial
// and not POD: a local closure variable is not initialized by a copy
// from a constant (clang `IsConstantAggregate`), and nothing is stored.
// From C++20 the captureless closure is trivial and gets the copy, as
// in clang (WP-1520).
//@ args: -std=c++17
template <class F> int call(F f) { return f(); }
int main() {
  auto l = [] { return 1; };
  auto m = l;
  int x = 2;
  auto c = [x] { return x; };
  return call(l) + call([] { return 2; }) + call(c) + m();
}
