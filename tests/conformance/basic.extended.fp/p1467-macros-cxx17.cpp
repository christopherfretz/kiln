// [cpp.predefined], [lex.fcon] (P1467R9): before C++23 the
// __STDCPP_FLOATN_T__ macros are not defined, but, as in GCC 14, the
// __FLT32_*__ ... __BFLT16_*__ limit macros are (libstdc++ uses them, and
// the literal suffixes they expand to, in every mode); the suffixes are
// accepted as an extension with a -Wc++23-extensions warning (pedantic, off
// by default, so this test is diagnostic-free).
//@ expect: compile-ok
//@ std: c++17
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no __FLT32_MANT_DIG__, no f32 suffix)
#if defined(__STDCPP_FLOAT16_T__) || defined(__STDCPP_FLOAT32_T__) || defined(__STDCPP_BFLOAT16_T__)
#error "__STDCPP_FLOATN_T__ is C++23"
#endif
#if __FLT32_MANT_DIG__ != 24 || __BFLT16_MANT_DIG__ != 8
#error "limit macros are defined in every C++ mode"
#endif
static_assert(sizeof(__FLT32_MAX__) == 4 && sizeof(__BFLT16_MAX__) == 2, "");
static_assert(sizeof(1.0f64) == 8 && sizeof(1.0f128) == 16, "");
