// [expr.const], [basic.extended.fp], [conv.double], [conv.fpint] (C++23,
// P1467R9): constant evaluation of _Float16, __bf16, _Float32, _Float64 and
// _Float128 arithmetic and conversions is IEEE-exact in each type's own
// format (round-to-nearest-even); bf16 and binary128 are not emulated
// through double. Every operation is stored before the next one, so excess
// precision (GCC 14 evaluates _Float16 and __bf16 in float) cannot change
// the result; the values agree with GCC 14.
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no _Float32/_Float64, no bf16/f128 suffixes)
template<class T> constexpr T add(T a, T b) { T r = a + b; return r; }
template<class T> constexpr T mul(T a, T b) { T r = a * b; return r; }
template<class T> constexpr T div(T a, T b) { T r = a / b; return r; }
template<class T> constexpr T sub(T a, T b) { T r = a - b; return r; }

// __bf16: 8-bit significand.
static_assert(add(1.0bf16, 0x1p-8bf16) == 1.0bf16);             // tie, to even
static_assert(add(0x1.02p0bf16, 0x1p-8bf16) == 0x1.04p0bf16);   // tie, to even (up)
static_assert(div(1.0bf16, 3.0bf16) == 0x1.56p-2bf16 && sub(1.0bf16, 0x1p-9bf16) == 1.0bf16);
static_assert(mul(0x1.fep120bf16, 0x1.02p0bf16) == 0x1p121bf16); // just below a tie
static_assert(mul(0x1p-126bf16, 0x1p-7bf16) == 0x1p-133bf16);  // subnormal
static_assert((__bf16)1.00390625f == 1.0bf16 && (__bf16)1.01171875f == 0x1.04p0bf16);
static_assert((int)(__bf16)301 == 300 && (int)(__bf16)303 == 304);
constexpr __bf16 tenth = 0.1bf16;
static_assert((float)tenth == 0x1.9ap-4f);
constexpr __bf16 nan_b = (__bf16)__builtin_nanf("");
static_assert(nan_b != nan_b);

// _Float16: 11-bit significand.
static_assert(add(2048.0f16, 1.0f16) == 2048.0f16 && add(2048.0f16, 3.0f16) == 2052.0f16);
static_assert(div(1.0f16, 3.0f16) == 0x1.554p-2f16);

// _Float32 and _Float64 behave as float and double.
static_assert(div(1.0f32, 3.0f32) == 1.0f / 3.0f && div(1.0f64, 3.0f64) == 1.0 / 3.0);
static_assert(add(16777216.0f32, 1.0f32) == 16777216.0f32);
static_assert(sub(0.3f64, 0.1f64) == 0.19999999999999998f64);
static_assert((_Float32)0.1f64 == 0.1f32 && (_Float64)0.1f32 == (double)0.1f);

// _Float128: 113-bit significand, no double emulation.
static_assert(add(1.0f128, 0x1p-100f128) != 1.0f128);
static_assert(add(1.0f128, 0x1p-113f128) == 1.0f128);              // tie, to even
static_assert(add(0x1.0000000000000000000000000001p0f128, 0x1p-113f128) == 0x1.0000000000000000000000000002p0f128);
static_assert(div(1.0f128, 3.0f128) == 0x1.5555555555555555555555555555p-2f128);
static_assert(mul(0.1f128, 3.0f128) == 0x1.3333333333333333333333333334p-2f128);
static_assert((double)div(1.0f128, 3.0f128) == 1.0 / 3.0);
static_assert((long long)0x1.fffffffffffffp62f128 == 0x7ffffffffffffc00LL);
static_assert((_Float128)(unsigned long long)-1 == 0x1.fffffffffffffffep63f128);
