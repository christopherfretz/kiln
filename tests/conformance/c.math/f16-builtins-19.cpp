// [c.math.syn], [cpp.cond], [expr.const]: at -fclang-compat=19 the
// `__fp16` builtins only clang 22 declares are not builtins (an
// undeclared identifier), and fmaxf16 / fminf16 fold as clang 19 folds
// them (`FloatExprEvaluator`: a NaN first operand, even a signaling one,
// yields the second operand unchanged; otherwise the larger / smaller,
// `-0 < +0`), bit-exactly. At 22: f16-builtins-22.cpp (WP-403).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 32:9 error "use of undeclared identifier '__builtin_atan2f16'"
//@ diag: 33:9 error "use of undeclared identifier '__builtin_fmaximum_numf16'"
static_assert(!__has_builtin(__builtin_atan2f16) && !__has_builtin(__builtin_sincosf16));
static_assert(!__has_builtin(__builtin_fmaximum_numf16) && !__has_builtin(__builtin_fminimum_numf16));

using u16 = unsigned short;
// A macro: x86_64 has no `__fp16` parameters.
#define B(h) __builtin_bit_cast(u16, (__fp16)(h))
constexpr __fp16 snan = __builtin_nansf16("5"); // 0x7c05
constexpr __fp16 qnan = __builtin_nanf16("3");  // 0x7e03
static_assert(B(__builtin_fmaxf16(snan, 1.0)) == 0x3c00);
static_assert(B(__builtin_fmaxf16(snan, qnan)) == 0x7e03);
static_assert(B(__builtin_fmaxf16(qnan, snan)) == 0x7c05);
static_assert(B(__builtin_fminf16(snan, qnan)) == 0x7e03);
static_assert(B(__builtin_fmaxf16(1.0, qnan)) == 0x3c00);
static_assert(B(__builtin_fmaxf16(-0.0, 0.0)) == 0x0000);
static_assert(B(__builtin_fminf16(0.0, -0.0)) == 0x8000);
static_assert(B(__builtin_fminf16(1.5, -2.5)) == 0xc100);
static_assert(__builtin_bit_cast(unsigned, __builtin_fmaxf(__builtin_nansf("5"), 1.0f)) == 0x3f800000);

__fp16 h;
void f() {
  (void)__builtin_atan2f16(h, h);
  (void)__builtin_fmaximum_numf16(h, h);
}
