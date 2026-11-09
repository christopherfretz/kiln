//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): neither clang 22 nor GCC 16 implements P2686R5 constexpr references to automatic objects
//@ diag: 23:17 error "static assertion expression is not an integral constant expression"
//@ diag: 23:17 note "read of variable 'a' whose value is not known"
//@ diag: 22:14 note "declared here"
//@ diag: 25:22 error "variable 'r' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 24:24 note "'r' declared here"
//@ diag: 25:3 note "lambda expression begins here"
//@ diag: 25:4 note "capture 'r' by value"
//@ diag: 25:4 note "capture 'r' by reference"
//@ diag: 25:4 note "default capture by value"
//@ diag: 25:4 note "default capture by reference"
//@ diag: 27:17 error "static assertion expression is not an integral constant expression"
//@ diag: 27:17 note "read of temporary is not allowed in a constant expression outside the expression that created the temporary"
//@ diag: 26:23 note "temporary created here"
// [expr.const.init]/2, [expr.const]/4,9 (C++26, P2686R5): only a
// constexpr reference makes its automatic temporary constexpr-referenceable
// (the non-constexpr case waits for an oracle, WP-514); a lambda body is
// another function parameter scope; a non-const temporary is not usable.
void f() {
  const int &a = 1;
  static_assert(a == 1);
  constexpr const int &r = 2;
  [] { static_assert(r == 2); }();
  constexpr int &&m = 3;
  static_assert(m == 3);
}
