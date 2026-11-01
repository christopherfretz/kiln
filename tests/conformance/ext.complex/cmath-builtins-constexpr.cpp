// The C99 <complex.h> builtins are never constant in clang 19 (WP-237),
// like the other libm builtins: [expr.const]/10 (an invocation of a
// non-constexpr function).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:22 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 11:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:31 error "constexpr variable 'e' must be initialized by a constant expression"
constexpr _Complex double z = 1.0;
constexpr double a = __builtin_cabs(z);
static_assert(__builtin_carg(z) == 0.0);
constexpr _Complex double e = __builtin_cexp(z);
