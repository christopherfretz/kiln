// [expr.const]/16: a consteval constructor; each construction is an
// immediate invocation, including in casts and copy-initialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:5 error "call to consteval function 'P::P' is not a constant expression"
//@ diag: 24:9 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:12 note "declared here"
//@ diag: 25:5 error "call to consteval function 'P::P' is not a constant expression"
//@ diag: 20:35 note "division by zero"
//@ diag: 25:5 note "in call to 'P(1, 0)'"
//@ diag: 26:12 error "call to consteval function 'P::P' is not a constant expression"
//@ diag: 26:14 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:12 note "declared here"
//@ diag: 27:10 error "call to consteval function 'P::P' is not a constant expression"
//@ diag: 27:12 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 22:12 note "declared here"
struct P {
  int v;
  consteval P(int x) : v(x) {}
  consteval P(int x, int y) : v(x / y) {}
};
P make(int n) {
  P ok(7);
  P bad(n);
  P zero(1, 0);
  P temp = P(n);
  return P{n};
}
int use() { P p = 3; return p.v + P(4).v; }
