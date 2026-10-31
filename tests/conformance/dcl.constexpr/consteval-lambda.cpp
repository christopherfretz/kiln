// [expr.prim.lambda.closure]/5, [expr.const]/18 (P2564): a consteval lambda,
// and a lambda call operator that becomes immediate by calling one.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:11 error "call to consteval function 'run(int)::(anonymous class)::operator()' is not a constant expression"
//@ diag: 19:13 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 16:13 note "declared here"
//@ diag: 22:11 error "call to immediate function 'run(int)::(anonymous class)::operator()' is not a constant expression"
//@ diag: 22:13 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 16:13 note "declared here"
//@ diag: 24:26 error "call to immediate function 'run(int)::(anonymous class)::operator()' is not a constant expression"
//@ diag: 23:28 note "read of non-constexpr variable 'g' is not allowed in a constant expression"
//@ diag: 24:26 note "in call to 'g.operator()()'"
//@ diag: 23:8 note "declared here"
consteval int sq(int x) { return x * x; }
int run(int n) {
  auto c = [](int y) consteval { return y + 1; };
  int a = c(2);
  int b = c(n);
  auto e = [](int y) { return sq(y); };
  int d = e(3);
  int f = e(n);
  auto g = [n] { return sq(n); };
  return a + b + d + f + g();
}
