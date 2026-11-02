// [basic.extended.fp], GNU _Complex extension, the call ABI of `_Complex`
// _Float32/_Float64/_Float128 (spec/abi.md 8.17): passed and returned as
// _Complex float/double/__float128 are (x86-64: <2 x float>, two SSE
// eightbytes, memory for the 32-byte pair; AArch64: HFAs of two), mangled
// CDF32_/CDF64_/CDF128_ as GCC 14 (the calls go through asm-label
// declarations of the GCC names, so a mangling mismatch fails to link); mixed with other arguments in
// non-inlined calls, and multiplied/divided at run time (Annex G
// __mul?c3/__div?c3) with the values g++ 14 prints.
//@ expect: run
//@ skip(clang): clang 19 has no _Float32/_Float64/_Float128 in C++
//@ stdout: -3 4 0.12 0.16 0 -2 1 0 6 2.5 0
using C32 = _Complex _Float32;
using C64 = _Complex _Float64;
using C128 = _Complex _Float128;
extern "C" int printf(const char *, ...);
[[gnu::noinline]] C32 mul(C32 x, C32 y) { return x * y; }
[[gnu::noinline]] C64 div(C64 x, C64 y) { return x / y; }
[[gnu::noinline]] C128 mul2(C128 x, C128 y) { return x * y; }
[[gnu::noinline]] C128 div2(C128 x, C128 y) { return x / y; }
[[gnu::noinline]] _Float64 mix(int i, C32 a, double d, C128 b, C64 c, float f) {
  return i + __real__ a + d + double(__imag__ b) + __real__ c + f;
}
[[gnu::noinline]] C64 swap(C64 z) { return C64{__imag__ z, __real__ z}; }
C32 mul_g(C32, C32) __asm__("_Z3mulCDF32_S_");
C64 div_g(C64, C64) __asm__("_Z3divCDF64_S_");
C128 mul2_g(C128, C128) __asm__("_Z4mul2CDF128_S_");
_Float64 mix_g(int, C32, double, C128, C64, float) __asm__("_Z3mixiCDF32_dCDF128_CDF64_f");
int main() {
  C32 a = {1, 2};
  C64 b = {3, 4}, i = {0, 1};
  C128 c = {1, -1};
  C32 p = mul_g(a, a);
  C64 q = div_g(i, swap(b));
  C128 r = mul2_g(c, c), s = div2(c, c);
  printf("%g %g %g %g %g %g %g %g %g %g %g\n", double(__real__ p), double(__imag__ p),
         double(__real__ q), double(__imag__ q), double(__real__ r), double(__imag__ r),
         double(__real__ s), double(__imag__ s), double(mix_g(1, a, 2, c, b, 0)),
         double(__imag__ mul(C32{0.5f32, 0}, C32{0, 5})), double(__real__ swap(i)) - 1);
}
