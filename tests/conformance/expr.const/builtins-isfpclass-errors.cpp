// [expr.const]: the mask of __builtin_isfpclass is an integer constant
// in [0, 1023] (reported at the call); the operand must be floating; the
// probability of __builtin_expect_with_probability is a constant floating
// value in [0.0, 1.0] (clang BuiltinFPClassification /
// BuiltinExpectWithProbability).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:33 error "argument to '__builtin_isfpclass' must be a constant integer"
//@ diag: 19:26 error "argument value 1024 is outside the valid range [0, 1023]"
//@ diag: 20:43 error "floating point classification requires argument of floating point type (passed in 'int')"
//@ diag: 21:47 error "too few arguments to function call, expected 2, have 1"
//@ diag: 22:75 error "probability argument to __builtin_expect_with_probability must be constant floating-point expression"
//@ diag: 22:75 note "function parameter 'p' with unknown value cannot be used in a constant expression"
//@ diag: 22:23 note "declared here"
//@ diag: 23:65 error "probability argument to __builtin_expect_with_probability is outside the range [0.0, 1.0]"
//@ diag: 24:26 error "argument value -1 is outside the valid range [0, 1023]"
//@ diag: 25:26 error "argument to '__builtin_isfpclass' must be a constant integer"
int a(double d, int n) { return __builtin_isfpclass(d, n); }
int b(double d) { return __builtin_isfpclass(d, 1024); }
int c(int i) { return __builtin_isfpclass(i, 1); }
int d(double x) { return __builtin_isfpclass(x); }
long e(long v, double p) { return __builtin_expect_with_probability(v, 1, p); }
long g(long v) { return __builtin_expect_with_probability(v, 1, 1.5); }
int h(double d) { return __builtin_isfpclass(d, -1); }
int k(double d) { return __builtin_isfpclass(d, 1.0); }
