// [expr.alignof] with GNU `__attribute__((aligned))` on a typedef named as
// an alias template's argument: clang 22 substitutes an alias template's
// arguments as written, so `AL<D>` is `D` and keeps the typedef's
// alignment (GCC 16.1 gives 8: it drops attributes on template
// arguments; clang 19 substituted the canonical arguments, 8). Inside a
// function template instantiated with `D`, `AL<U>` is `double`: the
// function template's argument is canonical. Found running GCC's C++
// testsuite.
//@ requires: clang>=22
//@ std: c++20
typedef double D __attribute__((aligned(32)));
template <class U> using AL = U;
template <class U> constexpr int f() { return alignof(AL<U>); }
static_assert(alignof(AL<D>) == 32);
static_assert(alignof(const AL<D>) == 32);
static_assert(f<D>() == 8);
static_assert(f<AL<D>>() == 8);
