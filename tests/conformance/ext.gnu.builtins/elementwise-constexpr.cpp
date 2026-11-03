// [expr.const]: clang 19 constant-evaluates none of
// __builtin_elementwise_abs / _max / _min: a call is not a constant
// expression (with "subexpression not valid" inside a constexpr function,
// no note at top level). elementwise-constexpr-clang22.cpp has clang 22.
//@ requires: clang<22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 36:40 note "subexpression not valid in a constant expression"
//@ diag: 37:15 note "in call to 'g(-4, 9)'"
//@ diag: 38:19 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 40:19 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:19 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 43:15 error "static assertion expression is not an integral constant expression"
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:39 note "subexpression not valid in a constant expression"
//@ diag: 46:15 note "in call to 'm(2.000000e+00)'"
//@ diag: 47:19 error "constexpr variable 'b' must be initialized by a constant expression"
typedef float v4f __attribute__((ext_vector_type(4)));
typedef int v4i __attribute__((ext_vector_type(4)));
static_assert(__builtin_elementwise_abs(-3) == 3);
static_assert(__builtin_elementwise_abs(-2147483647 - 1) == -2147483647 - 1);
static_assert(__builtin_elementwise_abs((short)-5) == 5);
static_assert(__builtin_elementwise_abs(-0.0) == 0.0);
static_assert(__builtin_elementwise_max(3u, 4000000000u) == 4000000000u);
static_assert(__builtin_elementwise_min(-3LL, 4LL) == -3LL);
constexpr int g(int x, int y) { return __builtin_elementwise_max(x, y); }
static_assert(g(-4, 9) == 9);
constexpr v4f a = __builtin_elementwise_abs((v4f){-1, 2, -3, 4});
static_assert(a[0] == 1 && a[2] == 3);
constexpr v4i c = __builtin_elementwise_abs((v4i){-1, 2, -2147483647 - 1, 4});
static_assert(c[2] == -2147483647 - 1 && c[0] == 1);
constexpr v4i d = __builtin_elementwise_min((v4i){-1, 2, -3, 4}, (v4i){0, 0, 0, 0});
static_assert(d[0] == -1 && d[1] == 0);
static_assert(__builtin_elementwise_max(1.0, 2.0) == 2.0);
constexpr double m(double x) { return __builtin_elementwise_max(x, 1.0); }
static_assert(m(2) == 2);
constexpr v4f b = __builtin_elementwise_max((v4f){-1, 2, -3, 4}, (v4f){0, 0, 0, 0});
