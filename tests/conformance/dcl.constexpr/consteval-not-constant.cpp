// [expr.const]/16: an immediate invocation that is not a constant expression
// is ill-formed; the notes say why. Nested and constant calls are fine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:11 error "call to consteval function 'sq' is not a constant expression"
//@ diag: 25:14 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 24:13 note "declared here"
//@ diag: 26:11 error "call to consteval function 'quot' is not a constant expression"
//@ diag: 18:45 note "division by zero"
//@ diag: 26:11 note "in call to 'quot(1, 0)'"
//@ diag: 27:11 error "call to consteval function 'sq' is not a constant expression"
//@ diag: 27:14 note "read of non-const variable 'global' is not allowed in a constant expression"
//@ diag: 23:5 note "declared here"
//@ diag: 29:11 error "call to consteval function 'M::scaled' is not a constant expression"
//@ diag: 29:20 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 24:13 note "declared here"
consteval int sq(int x) { return x * x; }
consteval int quot(int a, int b) { return a / b; }
struct M {
  int k;
  consteval int scaled(int y) const { return k * y; }
};
int global = 4;
int run(int n) {
  int a = sq(n);
  int b = quot(1, 0);
  int c = sq(global);
  constexpr M m{3};
  int d = m.scaled(n);
  int e = m.scaled(2) + sq(sq(2));
  return a + b + c + d + e;
}
