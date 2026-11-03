// [cpp.cond] / clang's `__has_constexpr_builtin` at -fclang-compat=19 for
// the P0533 builtins clang 22 made `Constexpr` ([c.math.syn], [c.math.abs],
// [c.math.fpclass]): clang 19 answers 0 for abs/labs/llabs, signbit and
// the six comparison macros although they are builtins, and kiln answers
// the same although its Oven folds them (their staging rows say
// `Oven::Yes`, WP-393; spec/pp.md "clang compatibility level").
// The constant-evaluation uses are has-constexpr-builtin-p0533-22.cpp.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
#define ZERO(b) static_assert(!__has_constexpr_builtin(b) && __has_builtin(b), #b)
ZERO(__builtin_abs); ZERO(__builtin_labs); ZERO(__builtin_llabs);
ZERO(__builtin_signbit);
// WP-419: the float / long double spellings.
ZERO(__builtin_signbitf); ZERO(__builtin_signbitl);
ZERO(__builtin_isgreater); ZERO(__builtin_isgreaterequal);
ZERO(__builtin_isless); ZERO(__builtin_islessequal);
ZERO(__builtin_islessgreater); ZERO(__builtin_isunordered);
#if __has_constexpr_builtin(__builtin_abs)
#error "__builtin_abs is constexpr only from clang 22"
#endif
