// [basic.extended.fp], [c.math.syn]: clang 19 and 22 provide no
// __builtin_*f32x / *f64x function (GCC's TS 18661-3 spellings), and
// neither does kiln, so __has_builtin answers 0 for each (spec/oven.md
// "_Float32x/_Float64x math builtins", WP-393). libstdc++ 14 names none;
// glibc's <math.h> names only the constant builtins, and because kiln
// reports clang's __GNUC__ 4 it maps them to the double / long double
// builtins (bits/floatn-common.h), whose constant folding the _Float32x /
// _Float64x constants and math use here. Reference: clang 22 (and 19).
//@ expect: compile-ok
//@ strict-diags
#define __STDC_WANT_IEC_60559_TYPES_EXT__ 1
#include <math.h>
#define NONE(b) static_assert(!__has_builtin(b) && !__has_constexpr_builtin(b), #b)
#define BOTH(f) NONE(__builtin_##f##f32x); NONE(__builtin_##f##f64x)
BOTH(fabs); BOTH(copysign); BOTH(fmax); BOTH(fmin); BOTH(ceil); BOTH(floor);
BOTH(trunc); BOTH(round); BOTH(rint); BOTH(nearbyint); BOTH(sqrt);
BOTH(nan); BOTH(nans); BOTH(inf); BOTH(huge_val); BOTH(fma); BOTH(fmod);
BOTH(frexp); BOTH(ldexp); BOTH(nextafter);

constexpr _Float32x big = HUGE_VAL_F32X;
constexpr _Float64x bigx = HUGE_VAL_F64X;
static_assert(big == __builtin_inf() && bigx == __builtin_infl());
constexpr _Float32x m = -2;
constexpr _Float64x mx = -2;
static_assert(__builtin_fabs(m) == 2 && __builtin_fabsl(mx) == 2);
static_assert(__builtin_copysign(1, m) == -1 && __builtin_copysignl(1, mx) == -1);
static_assert(__builtin_fmax(m, 1) == 1 && __builtin_fminl(mx, 1) == -2);
static_assert(__builtin_isinf(big));
static_assert(__builtin_isfinite(mx));
