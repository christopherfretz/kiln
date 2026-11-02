// [c.math.syn] (P0533 for every floating-point type), [basic.extended.fp]:
// the constexpr math builtins are exact in each format kiln has: the
// binary128 *f128 family, long double in its target format (x87 80-bit
// extended or binary128 or binary64, selected by __LDBL_MANT_DIG__ so the
// test is target-neutral), and _Float16/__bf16/_Float32/_Float64 through the
// float/double builtins they convert to (as libstdc++'s overloads do), with
// a single rounding back to the narrow format.
// Expectations from g++ 14, which folds the same calls except rint and
// nearbyint (it folds no rint form; kiln rounds to nearest-even, the only
// mode a constant expression sees), the pointer-output *f128 calls, and
// __builtin_nexttowardf128 (a kiln/clang spelling GCC lacks). The x87 block
// was checked against the format's definition (no x86-64 g++ here).
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19/22 constant-evaluate none of these builtins
// binary128.
static_assert(__builtin_ceilf128(0x1.0000000000000000000000000001p0f128) == 2);
static_assert(__builtin_floorf128(-0x1.ffffffffffffffffffffffffffffp111f128) == -0x1p112f128);
static_assert(__builtin_truncf128(-2.75f128) == -2 && __builtin_roundf128(2.5f128) == 3);
static_assert(__builtin_roundevenf128(2.5f128) == 2 && __builtin_rintf128(-3.5f128) == -4);
static_assert(__builtin_nearbyintf128(0.5f128) == 0 && __builtin_lroundf128(-2.5f128) == -3);
static_assert(__builtin_llrintf128(0x1p62f128) == 0x1p62 && __builtin_lrintf128(1.5f128) == 2);
static_assert(__builtin_fmodf128(0x1p16383f128, 3.0f128) == 2 && __builtin_remainderf128(7.0f128, 2.0f128) == -1);
constexpr int quo128(_Float128 x, _Float128 y) { int q = 0; __builtin_remquof128(x, y, &q); return q; }
static_assert(quo128(-7.0f128, 2.0f128) == -4);
static_assert(__builtin_fdimf128(3.0f128, 1.0f128) == 2);
static_assert(__builtin_fmaf128(1 + 0x1p-112f128, 1 + 0x1p-112f128, -(1 + 0x1p-111f128)) == 0x1p-224f128);
constexpr int fexp128(_Float128 x) { int e = 0; __builtin_frexpf128(x, &e); return e; }
static_assert(fexp128(0x1p-16494f128) == -16493 && fexp128(3.0f128) == 2);
static_assert(__builtin_ldexpf128(1.0f128, -16494) == 0x1p-16494f128 && __builtin_scalbnf128(1.0f128, 16383) == 0x1p16383f128);
static_assert(__builtin_scalblnf128(3.0f128, -1L) == 1.5f128);
static_assert(__builtin_ilogbf128(0x1p-16494f128) == -16494 && __builtin_logbf128(0x1p100f128) == 100);
constexpr _Float128 ip128(_Float128 x) { _Float128 i = 0; __builtin_modff128(x, &i); return i; }
static_assert(ip128(-0x1.8p100f128) == -0x1.8p100f128 && ip128(2.5f128) == 2);
static_assert(__builtin_nextafterf128(1.0f128, 2.0f128) == 1 + 0x1p-112f128);
static_assert(__builtin_nextafterf128(0x1p-16382f128, 1.0f128) == 0x1.0000000000000000000000000001p-16382f128);
static_assert(__builtin_nexttowardf128(1.0f128, 0.0L) == 1 - 0x1p-113f128);
static_assert(__builtin_signbit(-0.0f128) && __builtin_isless(1.0f128, 2.0f128));
static_assert(__builtin_ceilf128(-0.5f128) == 0 && __builtin_signbit(__builtin_ceilf128(-0.5f128)));

// long double, per target format.
#if __LDBL_MANT_DIG__ == 64
static_assert(__builtin_nextafterl(1.0L, 2.0L) == 1 + 0x1p-63L && __builtin_nextafterl(1.0L, 0.0L) == 1 - 0x1p-64L);
static_assert(__builtin_nextafterl(0x1p-16382L, 1.0L) == 0x1.0000000000000002p-16382L);
static_assert(__builtin_nextafterl(__builtin_infl(), 0.0L) == __LDBL_MAX__);
static_assert(__builtin_ldexpl(1.0L, -16445) == 0x1p-16445L && __builtin_ilogbl(0x1p-16445L) == -16445);
constexpr int fexpl(long double x) { int e = 0; __builtin_frexpl(x, &e); return e; }
static_assert(fexpl(0x1p-16445L) == -16444 && fexpl(__LDBL_MAX__) == 16384);
static_assert(__builtin_ceill(0x1.0000000000000002p0L) == 2 && __builtin_roundl(-0x1.fffffffffffffffep61L) == -0x1p62L);
static_assert(__builtin_fmal(1 + 0x1p-63L, 1 + 0x1p-63L, -(1 + 0x1p-62L)) == 0x1p-126L);
static_assert(__builtin_fmodl(__LDBL_MAX__, 3.0L) == 0 && __builtin_fmodl(0x1p16383L, 3.0L) == 2);
#elif __LDBL_MANT_DIG__ == 113
static_assert(__builtin_nextafterl(1.0L, 2.0L) == 1 + 0x1p-112L && __builtin_ilogbl(0x1p-16494L) == -16494);
static_assert(__builtin_ldexpl(1.0L, -16494) == 0x1p-16494L && __builtin_ceill(1 + 0x1p-112L) == 2);
static_assert(__builtin_fmal(1 + 0x1p-112L, 1 + 0x1p-112L, -(1 + 0x1p-111L)) == 0x1p-224L);
#elif __LDBL_MANT_DIG__ == 53
static_assert(__builtin_nextafterl(1.0L, 2.0L) == 1 + 0x1p-52L);
#endif
static_assert(__builtin_lroundl(2.5L) == 3 && __builtin_remainderl(5.0L, 2.0L) == 1.0L);
static_assert(__builtin_logbl(__LDBL_MIN__) == __LDBL_MIN_EXP__ - 1);

// _Float32/_Float64 are float/double layouts, reached through those builtins.
static_assert(__builtin_ceilf(1.25f32) == 2.0f32 && __builtin_floor(-1.25f64) == -2.0f64);
static_assert(__builtin_nextafterf(1.0f32, 2.0f32) == 1.0f32 + 0x1p-23f32);
static_assert(__builtin_fma(0.1f64, 10.0f64, -1.0f64) == 0x1p-54f64);

// _Float16/__bf16 through float: one rounding back to the narrow format.
static_assert(_Float16(__builtin_ceilf(1.0009765625f16)) == 2.0f16);
// 2^-9 + 2^-20 exactly in float, a tie in _Float16 that rounds to even.
static_assert(_Float16(__builtin_fmaf(1.0f16 + 0x1p-10f16, 1.0f16 + 0x1p-10f16, -1.0f16)) == 0x1p-9f16);
static_assert(__bf16(__builtin_truncf(-2.5bf16)) == -2.0bf16 && __bf16(__builtin_ldexpf(1.0bf16, -133)) == 0x1p-133bf16);
static_assert(__builtin_ilogbf(0x1p-24f16) == -24 && __builtin_ilogbf(0x1p-133bf16) == -133);
static_assert(__builtin_nextafterf16(1.0f16, 2.0f16) == 1.0f16 + 0x1p-10f16);
static_assert(__builtin_nextafterf16b(1.0bf16, 0.0bf16) == 1.0bf16 - 0x1p-8bf16);
