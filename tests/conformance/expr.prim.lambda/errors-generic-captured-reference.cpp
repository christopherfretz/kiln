// [expr.prim.lambda.capture]/8: in a generic lambda without a
// capture-default, a reference local whose initializer is not a constant
// expression must be captured in every specialization, so naming it is
// diagnosed in the template definition (clang
// `VariableCanNeverBeAConstantExpression`), as is the address of a
// constant local in a non-dependent full-expression; a constant local
// read in a dependent full-expression is not an odr-use (WP-581).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 28:36 error "variable 'rr' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 26:14 note "'rr' declared here"
//@ diag: 28:12 note "lambda expression begins here"
//@ diag: 28:13 note "capture 'rr' by value"
//@ diag: 28:13 note "capture 'rr' by reference"
//@ diag: 28:13 note "default capture by value"
//@ diag: 28:13 note "default capture by reference"
//@ diag: 30:41 error "variable 'n' cannot be implicitly captured in a lambda with no capture-default specified"
//@ diag: 27:13 note "'n' declared here"
//@ diag: 30:12 note "lambda expression begins here"
//@ diag: 30:13 note "capture 'n' by value"
//@ diag: 30:13 note "capture 'n' by reference"
//@ diag: 30:13 note "default capture by value"
//@ diag: 30:13 note "default capture by reference"
void h(int r) {
  const int& rr = r;
  const int n = 3;
  auto c = [](auto x) { return x + rr; };
  auto d = [](auto x) { return x + n; };
  auto e = [](auto x) { const int* p = &n; return x + *p; };
  (void)c; (void)d; (void)e;
}
