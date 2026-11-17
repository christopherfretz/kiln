// [class.compare.default]/1 (P0847R7): a defaulted comparison operator
// function with an explicit object parameter has two parameters, the
// object parameter first, of type `const C&` both or `C` both; by value
// (`this C, C`) is valid and compares as the `const C&` form does. Mixed
// forms are ill-formed. Found running GCC's C++ testsuite (clang 19 had
// required `const C&`; clang 22 agrees with the draft).
//@ requires: clang>=22
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:24 error "parameters for defaulted equality comparison operator must have the same type (found 'M' vs 'const M &')"
//@ diag: 17:24 error "parameters for defaulted equality comparison operator must have the same type (found 'const N &' vs 'N')"
#include <compare>
struct T { int v; auto operator<=>(this T, T) = default; };
struct U { int v; bool operator==(this U, U) = default; };
struct R { int v; bool operator==(this const R &, const R &) = default; };
struct M { int v; bool operator==(this M, const M &) = default; };
struct N { int v; bool operator==(this const N &, N) = default; };
static_assert((T{1} <=> T{2}) < 0);
static_assert(T{1} == T{1} && T{1} != T{2});
static_assert(U{1} != U{2} && U{2} == U{2});
static_assert(R{3} == R{3});
struct F { int v; friend bool operator==(F, F) = default; };
static_assert(F{1} == F{1});
