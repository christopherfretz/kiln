// [cpp.predefined]/2 (C++23, P1467R9): __STDCPP_FLOAT16_T__ ...
// __STDCPP_BFLOAT16_T__ mean the implementation supports std::float16_t ...
// std::bfloat16_t. libc++ (19 and 22) has no <stdfloat>, so under
// -stdlib=libc++ kiln leaves them undefined, as clang does; libc++'s `std`
// module would otherwise export the missing aliases (spec/pp.md, P1467
// macros). The GCC limit macros stay.
//@ expect: compile-ok
//@ std: c++23
//@ args: -stdlib=libc++
//@ strict-diags
#if defined(__STDCPP_FLOAT16_T__) || defined(__STDCPP_FLOAT32_T__) || defined(__STDCPP_FLOAT64_T__)
#error "__STDCPP_FLOATN_T__ defined with libc++"
#endif
#if defined(__STDCPP_FLOAT128_T__) || defined(__STDCPP_BFLOAT16_T__)
#error "__STDCPP_FLOAT128_T__ / __STDCPP_BFLOAT16_T__ defined with libc++"
#endif
int main() {}
