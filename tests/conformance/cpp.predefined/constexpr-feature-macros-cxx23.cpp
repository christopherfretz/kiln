// [cpp.predefined]/1, [tab:cpp.predefined.ft]: before C++26 neither
// P2747's nor P3068's feature-test value is advertised.
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
static_assert(__cpp_constexpr == 202211L);
#ifdef __cpp_constexpr_exceptions
#error "__cpp_constexpr_exceptions is C++26"
#endif
