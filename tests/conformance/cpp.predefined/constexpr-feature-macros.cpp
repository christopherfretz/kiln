// [cpp.predefined]/1 (Table 22), [tab:cpp.predefined.ft]: `__cpp_constexpr`
// is 202406L with P2747 (constexpr placement new) and
// `__cpp_constexpr_exceptions` is 202411L with P3068 (constexpr exceptions).
//@ skip(clang): clang 19 defines __cpp_constexpr 202306L, and neither clang 19 nor clang 22 defines __cpp_constexpr_exceptions (no P3068)
//@ expect: compile-ok
//@ strict-diags
static_assert(__cpp_constexpr == 202406L);
static_assert(__cpp_constexpr_exceptions == 202411L);
