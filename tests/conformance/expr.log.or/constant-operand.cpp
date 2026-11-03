// [expr.log.or]/1 (clang -Wconstant-logical-operand, on by default): a
// non-bool integer left operand of `||` with a right operand that folds to
// an integer warns, with one note suggesting `|` (no removal note, unlike
// `&&`). The same exemptions as for `&&` apply.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 25:10 warning "use of logical '||' with constant operand"
//@ diag: 25:10 note "use '|' for a bitwise operation"
//@ diag: 26:10 warning "use of logical '||' with constant operand"
//@ diag: 26:10 note "use '|' for a bitwise operation"
//@ diag: 27:10 warning "use of logical '||' with constant operand"
//@ diag: 27:10 note "use '|' for a bitwise operation"
//@ diag: 28:10 warning "use of logical '||' with constant operand"
//@ diag: 28:10 note "use '|' for a bitwise operation"
//@ diag: 29:10 warning "use of logical '||' with constant operand"
//@ diag: 29:10 note "use '|' for a bitwise operation"
//@ diag: 30:10 warning "use of logical '||' with constant operand"
//@ diag: 30:10 note "use '|' for a bitwise operation"
//@ diag: 30:14 warning "left operand of comma operator has no effect"
#define TWO 2
#define ZERO 0
enum E { e0, e1, e7 = 7 };
int f(int x, bool b, double d) {
  int r = 0;
  r += x || 2;
  r += x || 0;
  r += x || e1;
  r += x || TWO;
  r += x || __builtin_isinf_sign(-__builtin_inf());
  r += x || (x, 5);
  r += x || ZERO;
  r += x || false;
  r += b || 2;
  r += d || 2;
  r += x || e7;
  r += x || x;
  return r;
}
