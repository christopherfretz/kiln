// [basic.extended.fp] (C++23, P1467R9): an implementation that provides
// `std::float16_t` defines `__STDCPP_FLOAT16_T__`, the type is spelled by the
// `f16` literal suffix, and its conversion rank sits below `float`.
// WP-185 feature probe.
//@ expect: compile-ok
//@ std: c++23
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no __STDCPP_FLOAT16_T__)
//@ xfail(kiln): extended floating-point types are not implemented (P1467)
#ifndef __STDCPP_FLOAT16_T__
#error "no std::float16_t"
#endif
using f16 = decltype(1.0f16);
static_assert(sizeof(f16) == 2);
