// [expr.const], C23 7.6.2: the negative twin of fenv-round-constexpr.cpp.
// A directed `#pragma STDC FENV_ROUND` changes the value, so an assertion
// of the round-to-nearest value fails, and it does not make an invalid
// operation constant: division by zero and a NaN result still end the
// evaluation, as in clang 22.
//@ expect: compile-fail
//@ requires: clang>=22
//@ args: -std=c++26 -Wno-unknown-pragmas
//@ strict-diags
//@ diag: 20:15 error "static assertion failed due to requirement 'third == 0.33333333333333331'"
//@ diag: 20:21 note "expression evaluates to '0.33333333333333337 == 0.33333333333333331'"
//@ diag: 23:18 error "constexpr variable 'by_zero' must be initialized by a constant expression"
//@ diag: 23:32 note "division by zero"
//@ diag: 24:18 error "constexpr variable 'nan' must be initialized by a constant expression"
//@ diag: 24:28 note "floating point arithmetic produces a NaN"
//@ diag: 27:15 error "static assertion failed due to requirement 'narrow == 0.333333343F'"
//@ diag: 27:22 note "expression evaluates to '0.333333313 == 0.333333343'"
#pragma STDC FENV_ROUND FE_UPWARD
constexpr double third = 1.0 / 3.0;
static_assert(third == 0x1.5555555555555p-2);
constexpr double zero = 0.0;
constexpr double inf = __builtin_huge_val();
constexpr double by_zero = 1.0 / zero;
constexpr double nan = inf - inf;
#pragma STDC FENV_ROUND FE_DOWNWARD
constexpr float narrow = 1.0 / 3.0;
static_assert(narrow == 0x1.555556p-2f);
