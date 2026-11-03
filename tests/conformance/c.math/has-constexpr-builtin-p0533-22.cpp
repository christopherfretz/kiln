// [cpp.cond] / clang's `__has_constexpr_builtin` at -fclang-compat=22 for
// the P0533 builtins clang 22 made `Constexpr`, and a constant evaluation
// of each ([c.math.abs], [c.math.fpclass] signbit, [c.math.syn] the
// comparison macros, C23 7.12.17): the staging rows say `Oven::Yes`
// (WP-393), so the clang table answers 1 (spec/pp.md "clang compatibility
// level"). At 19 they answer 0: has-constexpr-builtin-p0533-19.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
#define ONE(b) static_assert(__has_constexpr_builtin(b) && __has_builtin(b), #b)
ONE(__builtin_abs); ONE(__builtin_labs); ONE(__builtin_llabs);
ONE(__builtin_signbit);
// WP-419: the float / long double spellings.
ONE(__builtin_signbitf); ONE(__builtin_signbitl);
ONE(__builtin_isgreater); ONE(__builtin_isgreaterequal);
ONE(__builtin_isless); ONE(__builtin_islessequal);
ONE(__builtin_islessgreater); ONE(__builtin_isunordered);
#if !__has_constexpr_builtin(__builtin_abs)
#error "__builtin_abs is constexpr from clang 22"
#endif
static_assert(__builtin_abs(-3) == 3);
static_assert(__builtin_labs(-3L) == 3L);
static_assert(__builtin_llabs(-3LL) == 3LL);
static_assert(__builtin_signbit(-0.0) && !__builtin_signbit(1.0f));
static_assert(__builtin_signbitf(-0.0f) && !__builtin_signbitf(1.0) && __builtin_signbitl(-1.0L));
static_assert(!__builtin_signbitl(__builtin_huge_vall()) && __builtin_signbitf(-__builtin_nanf("")));
static_assert(__builtin_isgreater(2.0, 1.0) && !__builtin_isgreater(__builtin_nan(""), 1.0));
static_assert(__builtin_isgreaterequal(1.0, 1.0));
static_assert(__builtin_isless(1.0f, 2.0f));
static_assert(__builtin_islessequal(1.0L, 1.0L));
static_assert(__builtin_islessgreater(1.0, 2.0) && !__builtin_islessgreater(1.0, 1.0));
static_assert(__builtin_isunordered(1.0, __builtin_nan("")) && !__builtin_isunordered(1.0, 2.0));
