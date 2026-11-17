// __builtin_ffsg, __builtin_clrsbg, __builtin_parityg and
// __builtin_assoc_barrier in constant expressions (GCC folds them; the
// never-constant __builtin_speculation_safe_value is in
// gcc-only-builtins-errors.cpp). Kiln also takes _BitInt operands, which GCC's
// C++ mode lacks (checked under kiln only, below). WP-1552.
//@ std: c++20
//@ expect: compile-ok
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have none of these builtins
//@ strict-diags
typedef int v4i __attribute__((vector_size(16)));

static_assert(__builtin_ffsg(0) == 0);
static_assert(__builtin_ffsg((signed char)-128) == 8);
static_assert(__builtin_ffsg((__int128)1 << 100) == 101);
static_assert(__builtin_clrsbg(0L) == 63);
static_assert(__builtin_clrsbg((short)-1) == 15);
static_assert(__builtin_clrsbg((short)0x3fff) == 1);
static_assert(__builtin_clrsbg((__int128)-2) == 126);
static_assert(__builtin_parityg(7u) == 1);
static_assert(__builtin_parityg((unsigned char)0x81) == 0);
static_assert(__builtin_parityg((unsigned __int128)3 << 90) == 0);
static_assert(__builtin_assoc_barrier(1.5 + 2) == 3.5);
static_assert(__builtin_assoc_barrier('a') == 97);

constexpr v4i vv = __builtin_assoc_barrier(v4i{1, 2, 3, 4} + 1);
static_assert(vv[0] == 2 && vv[3] == 5);

constexpr int arr[2] = {1, 2};
static_assert(*__builtin_assoc_barrier(arr + 1) == 2);

template <class T> constexpr int lead(T x) { return __builtin_clrsbg(x); }
static_assert(lead((signed char)1) == 6);

#ifdef __KILN__
static_assert(__builtin_clrsbg((_BitInt(37))0) == 36);
static_assert(__builtin_ffsg((_BitInt(70))1 << 69) == 70);
static_assert(__builtin_parityg((unsigned _BitInt(9))0x101) == 0);
#endif
