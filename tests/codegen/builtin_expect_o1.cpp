//@ args: -O1
// `__builtin_expect` and `__builtin_expect_with_probability` above -O0
// (GCC builtins; [intro.compliance.general]/11 extensions): the value and
// the expected value are emitted, then `llvm.expect.T(value, expected)`,
// or `llvm.expect.with.probability.T(value, expected, double P)` with the
// probability folded to a `double` constant (clang
// `Builtin::BI__builtin_expect`; at -O0 the value itself is the result,
// builtins_math.cpp). A condition converts to `long` first.
// `optnone` keeps the bodies.
#define ON __attribute__((optnone))
int side();
ON long f(long x) {
  if (__builtin_expect(x > 3, 1)) return 1;
  long y = __builtin_expect(x, side());
  bool b = __builtin_expect_with_probability(x == 2, 0, 0.25);
  return y + b + __builtin_expect_with_probability(x, 5L, 0.9f);
}
ON int g(int x) { return __builtin_expect(x, 0) ? 2 : 3; }
