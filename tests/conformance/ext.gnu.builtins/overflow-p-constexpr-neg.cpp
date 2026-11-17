// GNU __builtin_*_overflow_p: a third argument with side effects or a
// call is evaluated (GCC folds the call to `(r, flag)`), so a call to a
// non-constexpr function there is not a constant expression. Compared
// with GCC 16.1. WP-1549.
//@ std: c++20
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_*_overflow_p
int f();
static_assert(__builtin_mul_overflow_p(1 << 16, 1 << 16, f()));
