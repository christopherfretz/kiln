// [cpp.predefined]/1, [tab:cpp.predefined.ft] (P3074R7, P3726R2):
// `__cpp_trivial_union` is 202603L in C++26 (the DIS value, P3726R2's
// bump of P3074R7's 202502L). Neither clang 22 nor GCC 16.1 defines it
// (docs/STANDARD_SUPPORT.md "Macro gaps").
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not define __cpp_trivial_union (no P3074)
static_assert(__cpp_trivial_union == 202603L);
