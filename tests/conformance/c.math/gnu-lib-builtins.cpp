// [c.math.syn] (P0533: ceil, floor, trunc, round, fabs, fmin, fmax and
// copysign are constexpr since C++23) through libstdc++'s <cmath>: its
// double overloads are `using ::ceil;` etc., C library declarations that
// GCC folds as implicit builtins. Under -stdlib=libstdc++ kiln folds a
// call of these (and their f/l variants) as the __builtin_ twin
// (spec/sema.md §5.4 "GCC's implicit library builtins", WP-380).
// Reference: g++ 14 accepts this file; clang 19/22 reject every
// static_assert ("non-constexpr function 'ceil'").
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ args: -stdlib=libstdc++
//@ skip(clang): GCC is the reference; clang folds no C library function
#include <cmath>

static_assert(std::ceil(2.5) == 3.0 && std::ceil(-0.5) == 0.0);
static_assert(std::floor(-2.5) == -3.0 && ::floor(2.5) == 2.0);
static_assert(std::trunc(-2.5) == -2.0 && std::round(2.5) == 3.0);
static_assert(std::round(-0.5) == -1.0 && std::signbit(std::ceil(-0.5)));
static_assert(std::fabs(-2.0) == 2.0 && std::fmin(1.0, 2.0) == 1.0);
static_assert(std::fmax(1.0, 2.0) == 2.0 && std::fmax(-0.0, -1.0) == -0.0);
static_assert(std::copysign(1.0, -0.0) == -1.0);

// The f/l C library functions (and the libstdc++ overloads over them).
static_assert(::ceilf(1.5f) == 2.0f && ::floorl(1.5L) == 1.0L);
static_assert(::truncf(-1.5f) == -1.0f && ::roundl(0.5L) == 1.0L);
static_assert(::fabsl(-1.0L) == 1.0L && ::fminf(1.0f, 2.0f) == 1.0f);
static_assert(::fmaxl(1.0L, 2.0L) == 2.0L && ::copysignf(1.0f, -1.0f) == -1.0f);

// In a constexpr function, and as a template argument.
constexpr double up(double x) { return std::ceil(x) + std::floor(x); }
static_assert(up(1.25) == 3.0);
template <int N> struct I { static constexpr int v = N; };
static_assert(I<int(std::round(41.5))>::v == 42);

int main() {}
