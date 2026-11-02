//@ expect: ast
//@ ast-filter: test
//@ requires: clang>=22
// [expr.prim.lambda.capture]/12 (C++20 P1091/P1381): structured bindings can
// be captured, by copy or by reference, explicitly or implicitly. Inside the
// lambda the name refers to the enclosing binding (refers_to_enclosing) and
// keeps the binding's type.
struct P { int x, y; };
int test_capture(P p) {
  auto [a, b] = p;
  auto byref = [&] { return a + b; };
  auto bycopy = [a] { return a; };
  auto mixed = [=, &b] { return a * b; };
  return byref() + bycopy() + mixed();
}
