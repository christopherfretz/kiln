// [c.math.syn] (constexpr sqrt, P1383) through libstdc++'s <cmath>: its
// double sqrt is `using ::sqrt;`, the C library declaration GCC folds as
// an implicit builtin, and its float / long double overloads call
// __builtin_sqrtf / __builtin_sqrtl. Under -stdlib=libstdc++ kiln folds a
// call of ::sqrt / ::sqrtf / ::sqrtl as the __builtin_ twin, correctly
// rounded (spec/sema.md §5.4 "GCC's implicit library builtins", WP-393).
// Reference: g++ 14 accepts this file; clang 19/22 reject every
// static_assert ("non-constexpr function 'sqrt'").
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
//@ args: -stdlib=libstdc++
//@ skip(clang): GCC is the reference; clang folds no C library function
#include <cmath>

static_assert(std::sqrt(4.0) == 2.0 && std::sqrt(2.0) == 1.4142135623730951);
static_assert(::sqrt(0.25) == 0.5 && std::signbit(std::sqrt(-0.0)));
static_assert(std::sqrt(2.0f) == 0x1.6a09e6p0f && ::sqrtf(9.0f) == 3.0f);
static_assert(std::sqrt(0.25L) == 0.5L && ::sqrtl(16.0L) == 4.0L);
static_assert(std::sqrt(9) == 3.0);

constexpr double hyp(double a, double b) { return std::sqrt(a * a + b * b); }
static_assert(hyp(3, 4) == 5);

int main() {}
