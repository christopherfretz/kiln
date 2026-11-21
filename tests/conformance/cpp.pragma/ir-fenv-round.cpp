// [cfenv.syn], C23 7.6.2: in a `#pragma STDC FENV_ROUND` scope with a
// directed mode a function is `strictfp` and its FP operations are
// `llvm.experimental.constrained.*` calls carrying the mode
// (`round.upward`, `round.downward`, `round.towardzero`,
// `round.tonearestaway`) and `fpexcept.ignore` (`fpexcept.strict` under
// FENV_ACCESS ON); a block-scope pragma covers its block, a template its
// pattern's scope. Initializers and literals in the scope are folded in
// the mode. Compared with clang 22's IR exactly (spec/codegen.md
// "Floating-point semantics", WP-1744).
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=aarch64-unknown-linux-gnu -std=c++26 -Wno-unknown-pragmas
#pragma STDC FENV_ROUND FE_UPWARD
double g_third = 1.0 / 3.0;
float g_lit = 0.1f;
float g_int = (float)16777217;
double add(double a, double b) { return a + b; }
float narrow(double a) { return a; }
double itof(long long a) { return a; }
long long ftoi(double a) { return a; }
bool less(double a, double b) { return a < b; }
double inc(double a) { return ++a; }
double mul_assign(double a, double b) {
  a *= b;
  return a;
}
_Complex double cmul(_Complex double a, double b) { return a * b; }
_Complex double cmul2(_Complex double a, _Complex double b) { return a * b; }
double contract(double a, double b, double c) { return a * b + c; }
long double ldiv(long double a, int b) { return a / b; }
double neg(double a) { return -a; }
_Float16 half_add(_Float16 a, _Float16 b) { return a + b; }
__bf16 bf_mul(__bf16 a, __bf16 b) { return a * b; }
double callee(double);
double caller(double a) { return callee(a) * 2; }
double access(double a, double b) {
#pragma STDC FENV_ACCESS ON
  return a / b;
}
#pragma STDC FENV_ROUND FE_TONEARESTFROMZERO
double away(double a, double b) { return a - b; }
double g_tie = 0x1p53 + 1.0;
double scoped(double a, double b) {
  {
#pragma STDC FENV_ROUND FE_DOWNWARD
    a = a / b;
  }
  return a * b;
}
#pragma STDC FENV_ROUND FE_TONEAREST
double nearest(double a, double b) { return a + b; }
template <class T> T tadd(T a, T b) {
#pragma STDC FENV_ROUND FE_TOWARDZERO
  return a + b;
}
double use_tadd(double a, double b) { return tadd(a, b); }
