// [expr.const]/5.3, [c.math.syn]: the `__fp16` builtins of functions
// outside P0533 (the trigonometric, exponential, logarithmic and power
// functions) and canonicalizef16 are not constant expressions: as for
// their `f` siblings (and as in clang 19 and 22, which give no note), the
// Oven rejects the call (WP-403).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:22 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 12:22 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 13:22 error "constexpr variable 'c' must be initialized by a constant expression"
constexpr __fp16 a = __builtin_sinf16(1.5);
constexpr __fp16 b = __builtin_canonicalizef16(1.5);
constexpr __fp16 c = __builtin_powf16(1.5, 2);
