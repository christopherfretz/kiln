// [expr.add], [expr.ass]/6 under -ffp-contract=on: an `fmul` operand of a
// floating add/sub (also compound) is fused into `llvm.fmuladd` with the
// sign of each operand kept: `a + b*c` is fmuladd(b, c, a), `a - b*c`
// fmuladd(-b, c, a), `a -= b*c` likewise, `b*c - a` fmuladd(b, c, -a),
// and a negated product folds its fneg into the multiplicand.
double add_r(double a, double b, double c) { return a + b * c; }
double add_l(double a, double b, double c) { return b * c + a; }
double sub_r(double a, double b, double c) { return a - b * c; }
double sub_l(double a, double b, double c) { return b * c - a; }
double neg_r(double a, double b, double c) { return a + -(b * c); }
double neg_sub_r(double a, double b, double c) { return a - -(b * c); }
double neg_l(double a, double b, double c) { return -(b * c) - a; }
double cadd(double a, double b, double c) { a += b * c; return a; }
double csub(double a, double b, double c) { a -= b * c; return a; }
float fadd(float a, unsigned long x, float c) { a += x * c; return a; }
