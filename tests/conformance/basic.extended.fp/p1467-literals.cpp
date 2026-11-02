// [lex.fcon]/3 (C++23, P1467R9): a floating literal with an f16, f32, f64,
// f128 or bf16 suffix has the named extended type, and its value is the
// decimal value correctly rounded to that type (round-to-nearest-even), not
// double-rounded through float or double. Self-checking; every value agrees
// with GCC 14 except the first bf16 line: GCC 14 evaluates a bf16 literal in
// float (its excess precision) and then rounds that to bf16, so it gets 1.0
// there, a double rounding (spec/lex.md "Extended floating suffixes").
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no f32/f64/f128/bf16 suffixes)

// GCC evaluates _Float16 and __bf16 expressions, literals included, in float
// (excess precision, [expr.pre]/6), so each literal goes through V(), whose
// conversion to its own type discards any excess precision.
#define V(x) static_cast<decltype(x)>(x)

// bf16: 8-bit significand, ulp(1) = 2^-7. 1 + 2^-8 + 2^-30 is just above the
// half-way point, so it rounds up; rounding through float first would land
// on the exact tie 1 + 2^-8 and then round to even (down).
static_assert(V(1.003906250931322574615478515625bf16) == V(0x1.02p0bf16));
static_assert((__bf16)1.003906250931322574615478515625f == V(1.0bf16));
static_assert(V(1.00390625bf16) == V(1.0bf16));     // tie, to even (down)
static_assert(V(1.01171875bf16) == V(0x1.04p0bf16)); // tie, to even (up)
static_assert(V(0.1bf16) == V(0x1.9ap-4bf16));
static_assert(V(1e-40bf16) == V(0x1p-133bf16));      // subnormal
static_assert(V(3.3e38bf16) == V(0x1.f0p127bf16));

// f16: 11-bit significand.
static_assert(V(0.1f16) == V(0x1.998p-4f16) && V(65504.0f16) == V(0x1.ffcp15f16));
static_assert(V(2049.0f16) == V(2048.0f16) && V(2051.0f16) == V(2052.0f16));

// f32 and f64 round exactly like float and double literals.
static_assert(0.1f32 == 0.1f && 0.1F64 == 0.1);
static_assert(16777217.0f32 == 16777216.0f32 && 9007199254740993.0f64 == 9007199254740992.0f64);

// f128: 113-bit significand, rounded from the decimal value.
static_assert(0.1f128 == 0x1.999999999999999999999999999ap-4f128);
static_assert(0.1f128 != 0.1f64 && (double)0.1f128 == 0.1);
static_assert(1.0f128 + 0x1p-112f128 > 1.0f128 && 1.0f128 + 0x1p-114f128 == 1.0f128);
static_assert(1e4932f128 == 1e4932f128 && 1.18973149535723176508575932662800702e+4932f128 > 1e4932f128);
static_assert(3.3621031431120935062626778173217526e-4932f128 == 0x1p-16382f128);
