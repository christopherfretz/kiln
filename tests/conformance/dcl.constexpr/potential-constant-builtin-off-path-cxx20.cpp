// [dcl.constexpr]/6, [expr.const]/10: none of these constexpr functions is
// never constant under clang 19, though each calls a builtin clang does
// not constant-evaluate or a non-constexpr library builtin: the call is
// off the known path (an unknown condition, an unevaluated operand, an
// operand clang does not evaluate, a lambda body), a library builtin's
// argument is unknown, or the builtin is one clang evaluates (WP-945).
// spec/sema.md "Never constant".
//@ std: c++20
//@ strict-diags
//@ expect: compile-ok
extern "C" double fabs(double);
extern "C" int abs(int);
constexpr double v1(double x) { return fabs(x); }
constexpr int v2(int x) { return abs(x) + 1; }
constexpr int v3(int x) { return x ? int(__builtin_floor(1.0)) : 2; }
constexpr unsigned long v4(int x) { return sizeof(__builtin_floor(1.0)) + x; }
constexpr int v5(int x) {
  __builtin_assume(__builtin_floor(1.0) > 0);
  return x;
}
constexpr int v6(int x) { return __builtin_constant_p(__builtin_floor(x)) + x; }
constexpr long v7(long x) { return __builtin_expect(x, 1L); }
constexpr int v8(int x) {
  if (x)
    return 1;
  return int(__builtin_floor(1.0));
}
constexpr int v9(int x) {
  auto l = [] { return __builtin_floor(1.0); };
  (void)l;
  return x;
}
constexpr int v10(int x) {
  return __builtin_is_constant_evaluated() ? x : int(__builtin_floor(1.0));
}
constexpr int v11(unsigned x) {
  return __builtin_popcount(x) + __builtin_ffs(int(x)) + __builtin_clz(1u);
}
constexpr unsigned long v12(int x) { return __builtin_strlen("ab") + x; }
constexpr int v13(int x) { return __builtin_bit_cast(int, 1.0f) + x; }
template <class T> constexpr T v14(T x) { return x + __builtin_floor(1.0); }
constexpr int v15(int x) { return noexcept(__builtin_floor(1.0)) + x; }
