// [expr.const]: clang 22 constant-evaluates __builtin_elementwise_add_sat
// / _sub_sat (clang 20+ `Constexpr`): the exact sum or difference clamped
// to the operand type's range, by its signedness (bool and unsigned types
// clamp at 0 and the maximum), for every integer width up to 128 bits and
// lane-wise on vectors. elementwise-sat-constexpr.cpp has clang 19, which
// folds neither (WP-432).
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned char v4u8 __attribute__((ext_vector_type(4)));
enum E { e0, e1, e2 };
constexpr int IMAX = 2147483647, IMIN = -IMAX - 1;
static_assert(__builtin_elementwise_add_sat(IMAX, 1) == IMAX);
static_assert(__builtin_elementwise_add_sat(IMIN, -1) == IMIN);
static_assert(__builtin_elementwise_add_sat(3, 4) == 7);
static_assert(__builtin_elementwise_sub_sat(IMIN, 1) == IMIN);
static_assert(__builtin_elementwise_sub_sat(IMAX, -1) == IMAX);
static_assert(__builtin_elementwise_sub_sat(-5, 7) == -12);
static_assert(__builtin_elementwise_add_sat(4000000000u, 400000000u) == 4294967295u);
static_assert(__builtin_elementwise_sub_sat(3u, 4u) == 0u);
static_assert(__builtin_elementwise_add_sat((signed char)100, (signed char)100) == 127);
static_assert(__builtin_elementwise_sub_sat((signed char)-100, (signed char)100) == -128);
static_assert(__builtin_elementwise_add_sat((unsigned char)200, (unsigned char)100) == 255);
static_assert(__builtin_elementwise_add_sat(true, true) == true);
static_assert(__builtin_elementwise_sub_sat(false, true) == false);
static_assert(__builtin_elementwise_add_sat(e1, e2) == 3);
static_assert(__builtin_elementwise_add_sat(9223372036854775807LL, 1LL) == 9223372036854775807LL);
static_assert(__builtin_elementwise_sub_sat(-9223372036854775807LL - 1, 1LL) == -9223372036854775807LL - 1);
constexpr __int128 I128MAX = (__int128)(~(unsigned __int128)0 >> 1);
static_assert(__builtin_elementwise_add_sat(I128MAX, (__int128)1) == I128MAX);
static_assert(__builtin_elementwise_sub_sat(-I128MAX - 1, (__int128)1) == -I128MAX - 1);
static_assert(__builtin_elementwise_add_sat(~(unsigned __int128)0, (unsigned __int128)1) == ~(unsigned __int128)0);
static_assert(__builtin_elementwise_add_sat((_BitInt(7))60, (_BitInt(7))10) == 63);
static_assert(__builtin_elementwise_sub_sat((unsigned _BitInt(7))6, (unsigned _BitInt(7))10) == 0);
constexpr v4i a = {IMAX, IMIN, 1, -1}, b = {1, -1, 2, -2};
constexpr v4i s = __builtin_elementwise_add_sat(a, b);
static_assert(s[0] == IMAX && s[1] == IMIN && s[2] == 3 && s[3] == -3);
constexpr v4i d = __builtin_elementwise_sub_sat(a, -b);
static_assert(d[0] == IMAX && d[1] == IMIN && d[2] == 3 && d[3] == -3);
constexpr v4u8 c = {250, 5, 0, 128}, c2 = {10, 10, 1, 128};
constexpr v4u8 cs = __builtin_elementwise_sub_sat(c, c2);
static_assert(cs[0] == 240 && cs[1] == 0 && cs[2] == 0 && cs[3] == 0);
constexpr v4u8 ca = __builtin_elementwise_add_sat(c, c2);
static_assert(ca[0] == 255 && ca[1] == 15 && ca[2] == 1 && ca[3] == 255);
constexpr int g = __builtin_elementwise_add_sat(IMAX, IMAX);
// In a constexpr function, lane-wise over a loop.
constexpr int sum_sat(int n) {
  int s = 0;
  for (int i = 0; i < n; ++i)
    s = __builtin_elementwise_add_sat(s, IMAX / 4);
  return s;
}
static_assert(sum_sat(3) == 3 * (IMAX / 4));
static_assert(sum_sat(9) == IMAX);
