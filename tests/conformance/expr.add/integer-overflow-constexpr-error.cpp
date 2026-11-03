//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "constexpr variable 'h' must be initialized by a constant expression"
//@ diag: 15:21 note "value 2147483650 is outside the range of representable values of type 'int'"
//@ diag: 17:17 error "constexpr if condition is not a constant expression"
//@ diag: 17:19 note "value 4294967294 is outside the range of representable values of type 'int'"
//@ diag: 18:21 error "case value is not a constant expression"
//@ diag: 18:23 note "value 2147483648 is outside the range of representable values of type 'int'"
// [expr.const]/10.8 (an operation with undefined behaviour, [expr.pre]/4)
// in a manifestly constant-evaluated expression is an error, reported by
// the evaluation alone: no -Winteger-overflow warning joins it, for a
// `constexpr` variable's initializer, an `if constexpr` condition or a
// `case` value (clang's `CheckCompletedExpr` with `IsConstexpr`).
constexpr int g = 2147483647;
constexpr int h = g + 3;
void f(int x) {
  if constexpr (g * 2 < 0) {}
  switch (x) { case g + 1: break; }
}
