// [basic.extended.fp], [c.math.syn] (nextafter), C23 7.12.11.3: GCC's
// __builtin_nextafterf16 / __builtin_nextafterf16b at run time step the
// 16-bit encoding (no library entry point exists): every _Float16 and
// __bf16 value against seven directions is checked against an
// order-preserving integer reference, and a few edge results are printed
// as bits. NaN operands give a NaN; equal operands give y (so the sign of
// a zero y); a zero x gives the smallest subnormal with y's sign.
//@ expect: run
//@ strict-diags
//@ skip(clang): clang 19/22 do not declare __builtin_nextafterf16/f16b
//@ stdout: 0 0 3c01 3bff 0001 8001 0000 7bff 3f81 7f7f 8000
#include <cstdio>
#include <cstring>

template <class T> unsigned short bits(T v) {
  unsigned short h;
  std::memcpy(&h, &v, 2);
  return h;
}
template <class T> T of(unsigned short h) {
  T v;
  std::memcpy(&v, &h, 2);
  return v;
}

// The reference: NaN in, NaN out; x == y gives y; else one step of the
// sign-magnitude order (a zero result keeps x's sign).
template <class T> bool isnan16(unsigned short h, unsigned short inf) {
  return (h & 0x7fff) > inf;
}
template <class T> unsigned short ref(unsigned short hx, unsigned short hy, unsigned short inf) {
  if (isnan16<T>(hx, inf) || isnan16<T>(hy, inf)) return 0x7fff;
  int kx = hx & 0x8000 ? -(hx & 0x7fff) : hx & 0x7fff;
  int ky = hy & 0x8000 ? -(hy & 0x7fff) : hy & 0x7fff;
  if (kx == ky) return hy;
  int k = kx + (ky > kx ? 1 : -1);
  if (k == 0) return hx & 0x8000;
  return k < 0 ? 0x8000 | -k : k;
}

volatile int sink;

template <class T, class F> int sweep(F next, unsigned short inf) {
  const unsigned short dirs[] = {(unsigned short)(0x8000 | inf), 0xbc00, 0x8000, 0, 0x3c00, inf, 0x1234};
  int bad = 0;
  for (unsigned x = 0; x < 0x10000; ++x)
    for (unsigned short d : dirs) {
      unsigned short dy = d;
      if (inf == 0x7f80 && (d == 0xbc00 || d == 0x3c00)) dy = d == 0xbc00 ? 0xbf80 : 0x3f80;
      // Every 16-bit value is stored to a local before it is passed:
      // clang 19 (the second oracle) miscompiles a `__bf16` call result
      // passed straight on as an argument on x86-64 at -O0; kiln spills
      // it under LLVM 19 (spec/abi.md §8.18, bf16-call-result-x86_64-run).
      T a = of<T>(x), b = of<T>(dy);
      T n = next(a, b);
      unsigned short r = bits(n);
      unsigned short w = ref<T>(x, dy, inf);
      if (w == 0x7fff ? !isnan16<T>(r, inf) : r != w) ++bad;
    }
  return bad;
}

int main() {
  int a = sweep<_Float16>([](_Float16 x, _Float16 y) { return __builtin_nextafterf16(x, y); }, 0x7c00);
  int b = sweep<__bf16>([](__bf16 x, __bf16 y) { return __builtin_nextafterf16b(x, y); }, 0x7f80);
  volatile _Float16 one = 1.0f16, two = 2.0f16, zero = 0.0f16, nzero = -0.0f16, inf = __builtin_inff16();
  volatile __bf16 bone = 1.0bf16, btwo = 2.0bf16, binf = static_cast<__bf16>(__builtin_inff());
  volatile __bf16 bnzero = -0.0bf16, bzero = 0.0bf16;
  std::printf("%d %d %04x %04x %04x %04x %04x %04x %04x %04x %04x\n", a, b,
              bits(__builtin_nextafterf16(one, two)), bits(__builtin_nextafterf16(one, zero)),
              bits(__builtin_nextafterf16(zero, one)), bits(__builtin_nextafterf16(zero, -one)),
              bits(__builtin_nextafterf16(nzero, zero)), bits(__builtin_nextafterf16(inf, zero)),
              bits(__builtin_nextafterf16b(bone, btwo)), bits(__builtin_nextafterf16b(binf, bzero)),
              bits(__builtin_nextafterf16b(bzero, bnzero)));
}
