// [dcl.pre]/12, [expr.const]: a failed static_assert of a comparison notes
// what each side evaluates to, also when the condition is a full-expression
// with cleanups (a __builtin_bit_cast operand) or comes from a macro
// argument; floats print as APFloat::toString (literals `1.`, `2.F`; values
// `1`, `+Inf`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:15 error "static assertion failed due to requirement '__builtin_bit_cast(unsigned short, (__fp16)(1.)) == 15361'"
//@ diag: 36:14 note "expanded from macro 'B'"
//@ diag: 39:22 note "expression evaluates to '15360 == 15361'"
//@ diag: 40:15 error "static assertion failed due to requirement '__builtin_bit_cast(unsigned int, 1.F) == 0'"
//@ diag: 37:14 note "expanded from macro 'U'"
//@ diag: 40:23 note "expression evaluates to '1065353216 == 0'"
//@ diag: 42:15 error "static assertion failed due to requirement '__builtin_bit_cast(unsigned short, (__fp16)(h)) == 1'"
//@ diag: 36:14 note "expanded from macro 'B'"
//@ diag: 42:20 note "expression evaluates to '15360 == 1'"
//@ diag: 43:15 error "static assertion failed due to requirement '__builtin_bit_cast(unsigned short, (__fp16)1.) == 15361'"
//@ diag: 43:52 note "expression evaluates to '15360 == 15361'"
//@ diag: 44:17 error "static assertion failed due to requirement '2 + 1 == 4'"
//@ diag: 38:15 note "expanded from macro 'P'"
//@ diag: 44:23 note "expression evaluates to '3 == 4'"
//@ diag: 38:15 note "expanded from macro 'P'"
//@ diag: 47:15 error "static assertion failed due to requirement 'd == e'"
//@ diag: 47:17 note "expression evaluates to '1 == 2.5'"
//@ diag: 48:15 error "static assertion failed due to requirement 'big == 3.'"
//@ diag: 48:19 note "expression evaluates to '1.0000000000000001E+300 == 3'"
//@ diag: 49:15 error "static assertion failed due to requirement 'inf == 2.F'"
//@ diag: 49:19 note "expression evaluates to '+Inf == 2'"
//@ diag: 50:15 error "static assertion failed due to requirement '1. + 0 == 2.5E+10'"
//@ diag: 50:23 note "expression evaluates to '1 == 2.5E+10'"
//@ diag: 51:15 error "static assertion failed due to requirement '1.5F + 0 == 2.99999992E-5F'"
//@ diag: 51:24 note "expression evaluates to '1.5 == 2.99999992E-5'"
//@ diag: 52:15 error "static assertion failed due to requirement '0.10000000000000001 + 0 == 1.0E-300'"
//@ diag: 52:23 note "expression evaluates to '0.10000000000000001 == 1.0E-300'"
using u16 = unsigned short;
#define B(h) __builtin_bit_cast(u16, (__fp16)(h))
#define U(x) __builtin_bit_cast(unsigned, x)
#define P(x) (x)
static_assert(B(1.0) == 0x3c01);
static_assert(U(1.0f) == 0);
constexpr __fp16 h = 1.0;
static_assert(B(h) == 1);
static_assert(__builtin_bit_cast(u16, (__fp16)1.0) == 0x3c01);
static_assert(P(2 + 1 == 4));
constexpr double d = 1.0, e = 2.5, big = 1e300;
constexpr float inf = __builtin_inff();
static_assert(d == e);
static_assert(big == 3.0);
static_assert(inf == 2.0f);
static_assert(1.0 + 0 == 2.5e10);
static_assert(1.5f + 0 == 3e-5f);
static_assert(0.1 + 0 == 1e-300);
