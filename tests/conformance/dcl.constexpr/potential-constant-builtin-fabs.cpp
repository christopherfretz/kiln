// [dcl.constexpr]/6: __builtin_fabs is usable in constant expressions, so
// CmpF can produce one; clang does not warn (-Winvalid-constexpr). From
// clang's SemaCXX/constexpr-vectors.cpp. Found by the clang negative-test
// run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, WP-873 never-constant check fails on a builtin call with unknown arguments
//@ expect: compile-ok
constexpr auto CmpF(float t, float u) { return __builtin_fabs(t - u) < 0.0001; }
static_assert(CmpF(1.0f, 1.0f), "");
