// [c.math.syn], [c.math.abs] (constexpr signbit, the comparison macros, abs;
// P0533), C23 7.12.3.6, 7.12.17, 7.24.6.1: signbit reads the sign bit (of
// zeros and NaNs too); isgreater..islessgreater are the quiet comparisons
// (false on NaN, which isunordered reports) for every float kind; integer abs
// is exact except of the most negative value (constexpr-errors.cpp);
// copysignf16 works on _Float16. fpclassify/isnan/isinf/isfinite/isnormal
// are covered in expr.const/builtins-eval.cpp.
// Expectations from g++ 14, which folds the same calls.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 19 constant-evaluates none of these builtins
// clang -Wconstant-logical-operand (spec/sema.md §5.25): an int-valued
// builtin as the right operand of `&&` folds, as in clang 22.
//@ diag: 31:42 warning "use of logical '&&' with constant operand"
//@ diag: 31:42 note "use '&' for a bitwise operation"
//@ diag: 31:42 note "remove constant to silence this warning"
//@ diag: 36:48 warning "use of logical '&&' with constant operand"
//@ diag: 36:48 note "use '&' for a bitwise operation"
//@ diag: 36:48 note "remove constant to silence this warning"
//@ diag: 39:48 warning "use of logical '&&' with constant operand"
//@ diag: 39:48 note "use '&' for a bitwise operation"
//@ diag: 39:48 note "remove constant to silence this warning"
//@ diag: 41:43 warning "use of logical '&&' with constant operand"
//@ diag: 41:43 note "use '&' for a bitwise operation"
//@ diag: 41:43 note "remove constant to silence this warning"
constexpr double qnan = __builtin_nan("");

static_assert(__builtin_signbit(-0.0) && !__builtin_signbit(0.0) && __builtin_signbit(-qnan));
static_assert(__builtin_signbit(-1.0f) && !__builtin_signbit(1.0L) && __builtin_signbit(-__builtin_inf()));
static_assert(__builtin_signbit(-1.0f16) && !__builtin_signbit(1.0bf16) && __builtin_signbit(-0.0f128));
static_assert(__builtin_signbit(-1.0f32) && __builtin_signbit(-1.0f64));

static_assert(__builtin_isgreater(2.0, 1.0) && !__builtin_isgreater(1.0, 1.0) && !__builtin_isgreater(qnan, 1.0));
static_assert(__builtin_isgreaterequal(1.0f, 1.0f) && !__builtin_isgreaterequal(1.0, qnan));
static_assert(__builtin_isless(1.0L, 2.0L) && !__builtin_isless(-0.0, 0.0) && !__builtin_isless(qnan, qnan));
static_assert(__builtin_islessequal(-0.0, 0.0) && __builtin_islessequal(1.0f128, 1.0f128));
static_assert(__builtin_islessgreater(1.0, 2.0) && !__builtin_islessgreater(1.0, 1.0) && !__builtin_islessgreater(qnan, 1.0));
static_assert(__builtin_isunordered(qnan, 1.0) && !__builtin_isunordered(__builtin_inf(), 1.0));
static_assert(__builtin_isless(1.0f16, 2.0f16) && __builtin_isgreater(2.0bf16, 1.0bf16));
// Mixed operands compare after the usual arithmetic conversions.
static_assert(__builtin_isless(1.0f, 1.5) && __builtin_isgreater(0x1p-149f, 0.0L));

static_assert(__builtin_abs(-3) == 3 && __builtin_abs(__INT_MAX__) == __INT_MAX__);
static_assert(__builtin_abs(-__INT_MAX__) == __INT_MAX__);
static_assert(__builtin_labs(-7L) == 7 && __builtin_llabs(-__LONG_LONG_MAX__) == __LONG_LONG_MAX__);

static_assert(__builtin_copysignf16(1.5f16, -0.0f16) == -1.5f16);
static_assert(__builtin_copysignf16(-2.0f16, 1.0f16) == 2.0f16);
