//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ requires: clang>=23
//@ stdout: 3 7 1 3 7 11 3 11 7
// [expr.prim.lambda.capture]/8-11, [basic.def.odr]/5, [stmt.expand]/5:
// the range of an iterating or destructuring expansion statement in a
// lambda odr-uses the local it names (a reference binds to it), so a
// capture-default captures it, also in a generic lambda whose call
// operator is instantiated after its closure is complete; a use that is
// no odr-use (an element whose value is read, an unevaluated operand)
// and a `static constexpr` local need no capture. Found running GCC's
// C++ testsuite (the generic lambda was accepted without a capture and
// then failed in code generation). An element of a constexpr array
// (`a[0]`) is no odr-use of `a` either, but GCC 16.1 asks for a capture
// there, so this GCC-checked test does not use one.
#include <cstdio>

struct B { int x = 1, y = 2; };

int main() {
  constexpr B b;
  constexpr int a[] = {3, 4};
  static constexpr B sb{5, 6};
  auto c1 = [=](auto) { int s = 0; template for (auto e : b) { s += e; } return s; };
  auto c2 = [&](auto) { int s = 0; template for (auto e : a) { s += e; } return s; };
  auto c3 = [=](auto) { int s = 0; template for (const B& e : {b}) { s += e.x; } return s; };
  std::printf("%d %d %d", c1(0), c2(0), c3(0));
  // Not generic.
  auto d1 = [&] { int s = 0; template for (auto e : b) { s += e; } return s; };
  auto d2 = [=] { int s = 0; template for (auto e : a) { s += e; } return s; };
  // No odr-use: no capture needed, in either kind of lambda.
  auto n1 = [](auto) {
    int s = 0;
    template for (int e : {b.x, b.y, int(sizeof(b))}) { s += e; }
    return s;
  };
  auto n2 = [] { int s = 0; template for (int e : {b.y, b.x}) { s += e; } return s; };
  // A `static constexpr` local is no local entity to capture.
  auto n3 = [](auto) { int s = 0; template for (auto e : sb) { s += e; } return s; };
  std::printf(" %d %d %d %d %d", d1(), d2(), n1(0), n2(), n3(0));
  // Nested: both lambdas capture.
  auto m = [&] { return [=](auto) { int s = 0; template for (auto e : a) { s += e; } return s; }(0); };
  std::printf(" %d\n", m());
}
