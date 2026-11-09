// [dcl.constexpr]/6, [expr.const]/10: a call of a builtin clang evaluates
// in constant expressions, with arguments that depend on the parameters,
// does not make the function never constant; clang 19 accepts every
// function here without -Winvalid-constexpr. The callee of a direct call
// is not evaluated as a value (WP-962): string, math, bit, overflow,
// is_constant_evaluated and constant_p builtins.
//@ std: c++20
//@ strict-diags
//@ expect: compile-ok
constexpr unsigned long len(const char *p) { return __builtin_strlen(p); }
constexpr int cmp(const char *a, const char *b, unsigned long n) {
  return __builtin_memcmp(a, b, n) + __builtin_strncmp(a, b, n) + __builtin_strcmp(a, b);
}
constexpr const char *chr(const char *p, int c) { return __builtin_strchr(p, c); }
constexpr const void *mchr(const void *p, int c, unsigned long n) { return __builtin_memchr(p, c, n); }
constexpr double fm(double a, double b) {
  return __builtin_fmax(a, b) + __builtin_fmin(a, b) + __builtin_copysign(a, b) + __builtin_fabs(a);
}
constexpr float ff(float a) { return __builtin_fabsf(a) + __builtin_inff() + __builtin_nanf(""); }
constexpr bool fin(double d) { return __builtin_isnan(d) || __builtin_isinf(d) || __builtin_isfinite(d); }
constexpr int bits(unsigned x) {
  return __builtin_popcount(x) + __builtin_clz(x | 1) + __builtin_ctz(x | 1) + __builtin_parity(x);
}
constexpr unsigned long long rot(unsigned long long x) {
  return __builtin_bswap64(x) ^ __builtin_rotateleft64(x, 3);
}
constexpr bool ovf(int a, int b) {
  int r = 0;
  return __builtin_add_overflow(a, b, &r) || __builtin_mul_overflow(a, b, &r);
}
constexpr int ice(int x) { return __builtin_is_constant_evaluated() ? x : 0; }
constexpr int cp(int x) { return __builtin_constant_p(x) ? 1 : 2; }
constexpr long ex(long x) { return __builtin_expect(x, 0); }
constexpr const int *lau(const int *p) { return __builtin_launder(p); }
struct S {
  char buff[10] = {};
  constexpr bool operator==(const char *p) const { return __builtin_strcmp(p, buff) == 0; }
};
static_assert(len("abc") == 3);
static_assert(bits(8u) == 1 + 28 + 0 + 1);
static_assert(ice(4) == 4);
static_assert(fm(-1.0, 2.0) == 2.0 + -1.0 + 1.0 + 1.0);
static_assert(S{} == "");
