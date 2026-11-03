// [expr.const]: clang 22 constant-evaluates __builtin_elementwise_abs
// (integer abs wraps on the most negative value; floating abs is fabs)
// and the integer __builtin_elementwise_max / _min, scalar and lane-wise
// on vectors; its floating max / min are not constant and fail without a
// note. elementwise-constexpr.cpp has clang 19, which folds none.
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:15 error "constexpr variable 'b' must be initialized by a constant expression"
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
