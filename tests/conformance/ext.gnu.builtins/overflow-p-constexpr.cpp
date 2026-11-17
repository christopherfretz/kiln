// GNU __builtin_*_overflow_p in C++: a constant expression when its
// operands are (the third argument's value is never read, so a
// non-constexpr variable works there; one with side effects or a call is
// evaluated, so a constexpr call works and any other is not constant,
// overflow-p-constexpr-neg.cpp), usable in static_assert,
// a constexpr function, a template argument and `if constexpr`; at run
// time it evaluates the third argument's side effects. Compared with GCC
// 16.1 (clang 19/22 have no _p builtins). WP-1549.
//@ std: c++20
//@ expect: run
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_*_overflow_p
//@ strict-diags
//@ stdout: 1 0 1 2 1 1
extern "C" int printf(const char *, ...);
struct B { int s3 : 3; unsigned long long u40 : 40; } bf;
int x = 3;
constexpr int g() { return 0; }
static_assert(__builtin_add_overflow_p(__INT_MAX__, 1, 0));
static_assert(!__builtin_add_overflow_p(1, 2, x));
static_assert(__builtin_mul_overflow_p(1 << 16, 1 << 16, g()));
static_assert(__builtin_add_overflow_p(3, 1, bf.s3));
static_assert(!__builtin_mul_overflow_p(1LL << 39, 1, bf.u40));
static_assert(__builtin_mul_overflow_p(1LL << 40, 1, bf.u40));
template <class T> constexpr bool fits_sum(T a, T b) { return !__builtin_add_overflow_p(a, b, T()); }
static_assert(fits_sum<signed char>(100, 27) && !fits_sum<signed char>(100, 28));
static_assert(fits_sum(0u, ~0u) && !fits_sum(1u, ~0u));
template <bool B> struct Tag { static constexpr int v = B ? 1 : 2; };
static_assert(Tag<__builtin_sub_overflow_p(0u, 1u, 0u)>::v == 1);
static_assert(__is_same(decltype(__builtin_add_overflow_p(1, 2, 0)), bool));
template <class T> constexpr int kind() {
  if constexpr (__builtin_mul_overflow_p(T(1) << (sizeof(T) * 8 - 2), 2, T()))
    return 1;
  else
    return 2;
}
static_assert(kind<int>() == 1 && kind<unsigned>() == 2);
volatile int v = 1;
int main() {
  int y = 0;
  bool r = __builtin_add_overflow_p(v, __INT_MAX__, y++);
  bool s = __builtin_sub_overflow_p(v, 1, bf.s3 = 2);
  printf("%d %d %d %d %d %d\n", r, s, y, bf.s3, fits_sum<int>(v, 0), kind<long>());
}
