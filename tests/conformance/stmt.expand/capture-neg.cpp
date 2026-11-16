// [expr.prim.lambda.capture]/8, [stmt.expand]/5: a range naming a local of
// the enclosing function that the lambda does not capture is one error, the
// capture error; the hidden `__range` (iterating) or `[__u...]`
// (destructuring) variable it initializes is marked invalid, not diagnosed
// again for its failed constant initialization (GCC 16.1 also reports one
// error each).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
//@ diag: 34:61 error "variable 'r' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 32:15 note "'r' declared here"
//@ diag: 34:13 note "lambda expression begins here"
//@ diag: 34:14 note "capture 'r' by value"
//@ diag: 34:14 note "capture 'r' by reference"
//@ diag: 34:14 note "default capture by value"
//@ diag: 34:14 note "default capture by reference"
//@ diag: 35:61 error "variable 'b' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 33:15 note "'b' declared here"
//@ diag: 35:13 note "lambda expression begins here"
//@ diag: 35:14 note "capture 'b' by value"
//@ diag: 35:14 note "capture 'b' by reference"
//@ diag: 35:14 note "default capture by value"
//@ diag: 35:14 note "default capture by reference"
struct R {
  int a[2] = {1, 2};
  constexpr const int *begin() const { return a; }
  constexpr const int *end() const { return a + 2; }
};
struct B { int x = 1, y = 2; };
int f() {
  constexpr R r;
  constexpr B b;
  auto l1 = [] { int s = 0; template for (constexpr int x : r) { s += x; } return s; };
  auto l2 = [] { int s = 0; template for (constexpr int x : b) { s += x; } return s; };
  return l1() + l2();
}
