// [expr.prim.lambda.capture]/11: an id-expression in a lambda body that is
// an odr-use of an entity captured by copy names the closure's member, and
// [expr.const]/10 makes such an odr-use non-constant in an expression
// evaluated in the body outside a call of the call operator (the example
// there: `constexpr int j = *&n;`). Reading the member, directly or through
// a reference or `this`, is diagnosed; its address is not constant (as
// for the variable, clang's note). Siblings: explicit `[x]`, `[=]`, a
// generic lambda, `static_assert`. clang 19 and 22 read the variable and
// accept all but the address; GCC 16.1 rejects these. The forms both accept
// (a read through a reference parameter or `this`, a capture of a capture,
// a structured binding read) read the variable in kiln too (WP-1655:
// constexpr-call-reads-by-copy-capture-run.cpp, -binding-run.cpp). Found
// running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ std: c++26
//@ xfail(clang): clang reads the enclosing variable
//@ diag: 36:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 36:37 note "kiln: read of variable 'n' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 34:13 note "declared here"
//@ diag: 37:33 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 37:37 note "kiln: read of variable 'x' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 35:17 note "declared here"
//@ diag: 38:39 error "constexpr variable 'j' must be initialized by a constant expression"
//@ diag: 38:43 note "kiln: read of variable 'x' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 35:17 note "declared here"
//@ diag: 39:33 error "static assertion expression is not an integral constant expression"
//@ diag: 39:33 note "kiln: read of variable 'n' captured by copy is not allowed in a constant expression outside a call of the lambda"
//@ diag: 34:13 note "declared here"
//@ diag: 40:40 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 40:40 note "pointer to 'n' is not a constant expression"
//@ diag: 34:13 note "declared here"
void g() {
  const int n = 1;
  constexpr int x = 2;
  auto l1 = [=] { constexpr int j = *&n; return j; };
  auto l2 = [x] { constexpr int j = *&x; return j; };
  auto l6 = [=](auto) { constexpr int j = *&x; return j; };
  auto l8 = [=] { static_assert(*&n == 1); };
  auto l9 = [=] { constexpr const int *p = &n; return p; };
  (void)l1; (void)l2; (void)l6(0); (void)l8; (void)l9;
}
