// [cpp.predefined]/2, [basic.extended.fp] (C++23, P1467R9): an implementation
// that provides std::float16_t, float32_t, float64_t, float128_t and
// bfloat16_t defines __STDCPP_FLOAT16_T__ ... __STDCPP_BFLOAT16_T__ as 1.
// kiln also defines GCC 14's __FLT32_*__, __FLT64_*__, __FLT128_*__ and
// __BFLT16_*__ limit macros, which libstdc++'s <limits> and <cmath> consume;
// the values below are GCC 14's (identical on x86-64 and AArch64).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ xfail(clang): neither clang 19 nor clang 22 implements P1467 (no __STDCPP_FLOAT32_T__)
#if __STDCPP_FLOAT16_T__ != 1 || __STDCPP_FLOAT32_T__ != 1 || __STDCPP_FLOAT64_T__ != 1
#error "missing __STDCPP_FLOATN_T__"
#endif
#if __STDCPP_FLOAT128_T__ != 1 || __STDCPP_BFLOAT16_T__ != 1
#error "missing __STDCPP_FLOAT128_T__ / __STDCPP_BFLOAT16_T__"
#endif
#if __FLT32_MANT_DIG__ != 24 || __FLT64_MANT_DIG__ != 53 || __FLT128_MANT_DIG__ != 113 || __BFLT16_MANT_DIG__ != 8
#error "MANT_DIG"
#endif
#if __FLT32_DIG__ != 6 || __FLT64_DIG__ != 15 || __FLT128_DIG__ != 33 || __BFLT16_DIG__ != 2
#error "DIG"
#endif
#if __FLT128_MAX_EXP__ != 16384 || __FLT128_MIN_EXP__ != (-16381) || __BFLT16_MAX_EXP__ != 128
#error "MAX_EXP"
#endif
#if __FLT16_IS_IEC_60559__ != 1 || __FLT128_IS_IEC_60559__ != 1 || __BFLT16_IS_IEC_60559__ != 0
#error "IS_IEC_60559"
#endif
#ifdef __FLT32X_MANT_DIG__
#error "kiln has no _Float32x"
#endif
template<class T, class U> constexpr bool same = false;
template<class T> constexpr bool same<T, T> = true;
// The value macros carry the type's own suffix.
static_assert(same<decltype(__FLT32_MAX__), _Float32> && same<decltype(__FLT64_MIN__), _Float64>);
static_assert(same<decltype(__FLT128_EPSILON__), _Float128> && same<decltype(__BFLT16_MAX__), __bf16>);
static_assert(__FLT32_MAX__ == (_Float32)__FLT_MAX__ && __FLT64_EPSILON__ == (_Float64)__DBL_EPSILON__);
static_assert(__BFLT16_EPSILON__ == 0x1p-7bf16 && __BFLT16_MAX__ == 0x1.fep127bf16);
static_assert(__BFLT16_MIN__ == 0x1p-126bf16 && __BFLT16_DENORM_MIN__ == 0x1p-133bf16);
static_assert(__FLT128_EPSILON__ == 0x1p-112f128 && __FLT128_MIN__ == 0x1p-16382f128);
static_assert(__FLT128_DENORM_MIN__ == 0x1p-16494f128);
static_assert(__FLT128_MAX__ == 0x1.ffffffffffffffffffffffffffffp16383f128);
