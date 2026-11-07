//@ expect: run
//@ strict-diags
//@ stdout: 16 1 2 4 8 8 8 2 4 8 16
//@ stdout: s 1 0 1
//@ stdout: hi 3 lo fffffffffffffffc
//@ stdout: -1 255
//@ stdout: 2.5 0.5
// GCC "Common Type Attributes", mode (clang `handleModeAttr`): the
// declared integer or floating-point type is replaced by the type of the
// named machine mode's width, keeping its signedness.
// mbedtls declares its double-word limb `unsigned int __attribute__((
// mode(TI)))` for a 64x64->128-bit multiply; kiln ignored the attribute,
// so the limb was 32 bits and every bignum product was wrong.
#include <cstdio>

typedef unsigned int u128 __attribute__((mode(TI)));
typedef int s128 __attribute__((__mode__(__TI__)));
typedef int s8 __attribute__((mode(QI)));
typedef unsigned u16 __attribute__((__mode__(__HI__)));
typedef long s32 __attribute__((mode(SI)));
typedef int s64 __attribute__((mode(DI)));
typedef int w __attribute__((mode(word)));
typedef unsigned p __attribute__((mode(pointer)));
typedef float f16 __attribute__((mode(HF)));
typedef double f32 __attribute__((mode(SF)));
typedef float f64 __attribute__((mode(DF)));
typedef double f128 __attribute__((mode(TF)));

template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
static_assert(same<u128, unsigned __int128>::value);
static_assert(same<s8, signed char>::value);
static_assert(same<s64, long>::value);
static_assert(same<f32, float>::value);
static_assert(same<f128, long double>::value);

u128 mul(unsigned long a, unsigned long b) { return (u128)a * b; }

int main() {
  std::printf("%zu %zu %zu %zu %zu %zu %zu %zu %zu %zu %zu\n", sizeof(u128), sizeof(s8),
              sizeof(u16), sizeof(s32), sizeof(s64), sizeof(w), sizeof(p), sizeof(f16),
              sizeof(f32), sizeof(f64), sizeof(f128));
  s128 neg = -1;
  std::printf("s %d %d %d\n", (int)(neg < 0), (int)((u128)neg < 0), (int)(neg >> 100 == -1));
  u128 x = mul(~0ul, 4);
  std::printf("hi %llx lo %llx\n", (unsigned long long)(x >> 64), (unsigned long long)x);
  s8 c = (s8)255;
  unsigned char __attribute__((mode(QI))) uc = 255;
  std::printf("%d %d\n", c, uc);
  f64 d = 5.0;
  f16 h = 0.5f;
  std::printf("%.1f %.1f\n", (double)(d / 2), (double)h);
}
