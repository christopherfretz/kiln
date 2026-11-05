// [cpp.cond] (C++17, P0061R1): `__has_include` in #if and as a macro
// test. WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
#if !defined(__has_include)
#error __has_include is not defined
#endif
#if __has_include("p0061-probe.cpp") && __has_include(<stddef.h>)
static_assert(true);
#else
#error __has_include did not find an existing header
#endif
#if __has_include("no/such/header-p0061.h") || __has_include(<no_such_p0061>)
#error __has_include found a missing header
#endif
#define HDR <stddef.h>
#if !__has_include(HDR)
#error __has_include does not expand its macro operand
#endif
