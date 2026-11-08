//@ expect: run
//@ strict-diags
//@ stdout: 8 16 32 32
//@ stdout: 1.5 -2.5
//@ stdout: 2 0.25
// GCC mode with a complex mode (clang `handleModeAttr`): on a complex
// floating-point type, `SC`/`DC`/`TC` give the complex type of the 32-,
// 64- and 128-bit floating-point mode, and a real floating-point mode gives
// that real type. glibc's <bits/floatn.h> declares, on x86-64,
// `typedef _Complex float __cfloat128 __attribute__ ((__mode__ (__TC__)));`:
// kiln rejected TC ("unsupported machine mode 'TC'"), so every TU that
// included <stdlib.h> failed there. Host-independent; the x86-64 types are
// checked by mode-complex-x86.cpp.
#include <cstdio>

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

typedef _Complex float c32 __attribute__((mode(SC)));
typedef _Complex float c64 __attribute__((mode(DC)));
typedef _Complex float __cfloat128 __attribute__((__mode__(__TC__)));
typedef _Complex double r64 __attribute__((mode(DF)));
typedef const _Complex float cc32 __attribute__((mode(SC)));
static_assert(same<c32, _Complex float>::value);
static_assert(same<c64, _Complex double>::value);
static_assert(same<r64, double>::value);
static_assert(same<cc32, _Complex float>::value);
#if __LDBL_MANT_DIG__ == 113
static_assert(same<__cfloat128, _Complex long double>::value);
#endif

int main() {
  __cfloat128 z = 0;
  __real__ z = 1.5;
  __imag__ z = -2.5;
  std::printf("%zu %zu %zu %zu\n", sizeof(c32), sizeof(c64), sizeof(__cfloat128),
              sizeof(z * z));
  std::printf("%.1f %.1f\n", (double)__real__ z, (double)__imag__ z);
  c64 w = 0;
  __real__ w = 4;
  c64 q = w * w;
  r64 x = 0.5;
  std::printf("%.0f %.2f\n", (double)__real__ q / 8, (double)(x * x));
}
