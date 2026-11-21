// [GNU] __builtin_add_overflow / _sub_ / _mul_: the result object holds
// the infinite-precision result wrapped to its own type, also a
// _BitInt(N) result in a constant expression where an operand is wider
// than the result (as kiln and clang compute at run time). x86-64 only
// (aarch64's __BITINT_MAXWIDTH__ is 128). Deviation (docs/status/
// deviations.md): clang 19 and 22 store the unwrapped value in constant
// evaluation, so a later comparison sees the bits above the result's
// width; GCC has no _BitInt in C++.
//@ args: --target=x86_64-unknown-linux-gnu
//@ expect: compile-ok
//@ skip(clang): clang's constant evaluation stores the unwrapped value
using R = unsigned _BitInt(129);
constexpr bool f() {
  R res = 0;
  bool o = __builtin_mul_overflow(-1, (unsigned _BitInt(256))1 << 140, &res);
  return o && res == 0;
}
static_assert(f());
constexpr R g() {
  R res = 7;
  __builtin_mul_overflow(-1, (unsigned _BitInt(256))1 << 140, &res);
  return res;
}
static_assert(g() == 0);
constexpr R h() {
  R res = 7;
  __builtin_add_overflow((unsigned _BitInt(256))3 << 128, 1, &res);
  return res;
}
static_assert(h() == ((R)1 << 128 | 1));
