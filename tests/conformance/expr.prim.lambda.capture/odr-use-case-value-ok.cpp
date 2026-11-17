// [expr.prim.lambda.capture]/11, [basic.def.odr]/5, [stmt.switch]/2,
// accepted side of odr-use-case-value.cpp: a case value that names a
// captured constant without odr-using it reads the variable (the
// lvalue-to-rvalue conversion applied to it directly, in parentheses, as
// an operand of a built-in operator, in a conditional, at either end of a
// GNU case range); a by-reference capture names the variable itself, so
// `*&i` is constant there; a constant not captured at all is read in a
// lambda without a capture-default. The value is completed as a
// full-expression before it is evaluated: a temporary with a constexpr
// destructor and an immediate invocation in it still work. Found running
// GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++26
struct D {
  int v;
  constexpr ~D() {}
};
consteval int twice(int x) { return 2 * x; }
void f(int n) {
  constexpr int i = 1;
  auto l1 = [=](int k) {
    switch (k) {
    case i: case (i) + 1: case i + 2: case true ? i + 3 : 0: return i;
    case 10 ... 10 + i: return 2;
    }
    return 0;
  };
  auto l2 = [&](int k) { switch (k) { case *&i: return 1; } return 0; };
  auto l3 = [](int k) { switch (k) { case i: return 1; } return 0; };
  auto l4 = [=](auto k) { switch (k) { case i: case D{4}.v: return 1; } return 0; };
  auto l5 = [i](int k) { switch (k) { case twice(i): return 1; } return k + i; };
  (void)l1(0); (void)l2(0); (void)l3(0); (void)l4(0); (void)l5(0);
  switch (n) {
  case D{5}.v: case twice(3): break;
  }
}
