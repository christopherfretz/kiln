// [cpp.cond] / clang's `__has_constexpr_builtin` for the P0533 <cmath>
// builtins ([c.math.syn]): kiln answers clang's `Constexpr` table
// (Builtins.td), not the larger set its Oven evaluates (WP-365, GCC's
// policy: constexpr-rounding.cpp). clang 19 and 22 answer 0 for the
// rounding, sqrt and remainder families and 1 for fabs, fmin, fmax,
// copysign and the classification builtins; clang 22 adds signbit, abs
// and the isgreater family (WP-380, spec/pp.md "Clang compatibility
// level", `__has_constexpr_builtin` paragraph).
//@ expect: compile-ok
//@ strict-diags
#define ZERO(b) static_assert(!__has_constexpr_builtin(b) && __has_builtin(b), #b)
#define ONE(b) static_assert(__has_constexpr_builtin(b), #b)
ZERO(__builtin_ceil); ZERO(__builtin_ceilf); ZERO(__builtin_ceill);
ZERO(__builtin_floor); ZERO(__builtin_trunc); ZERO(__builtin_round);
ZERO(__builtin_nearbyint); ZERO(__builtin_rint); ZERO(__builtin_sqrt);
ZERO(__builtin_fmod); ZERO(__builtin_frexp); ZERO(__builtin_ldexp);
ZERO(__builtin_logb); ZERO(__builtin_ilogb); ZERO(__builtin_scalbn);
ZERO(__builtin_modf); ZERO(__builtin_remainder); ZERO(__builtin_remquo);
ZERO(__builtin_fdim); ZERO(__builtin_nextafter);
ONE(__builtin_fabs); ONE(__builtin_fabsf); ONE(__builtin_fabsl);
ONE(__builtin_fmin); ONE(__builtin_fminf); ONE(__builtin_fmax);
ONE(__builtin_fmaxl); ONE(__builtin_copysign); ONE(__builtin_copysignf);
ONE(__builtin_isnan); ONE(__builtin_isinf); ONE(__builtin_isfinite);
ONE(__builtin_fpclassify);
#define AT22(b) static_assert(__has_constexpr_builtin(b) == (__clang_major__ >= 22), #b)
AT22(__builtin_signbit); AT22(__builtin_abs); AT22(__builtin_llabs);
AT22(__builtin_isgreater); AT22(__builtin_islessgreater);
AT22(__builtin_isunordered);
#if __has_constexpr_builtin(__builtin_ceil)
#error "__builtin_ceil is not in clang's constexpr table"
#endif
