// [dcl.attr] `__attribute__((target("...")))` enables features for one
// function, as clang's `getFunctionFeatureMap`: under the default
// `x86-64` CPU, a `target("sse4.2")` / `target("popcnt")` /
// `target("aes")` / `target("pclmul")` function may call the intrinsics
// those features gate (always-inline header functions and builtins
// alike), and the header builtins `_mm_pause`, `_mm_mfence`, `_mm_lfence`,
// `_mm_sfence`, `_mm_clflush`, `_mm_getcsr` and `_mm_setcsr` lower to their
// intrinsics (spec/landed/codegen/WP-630.md "Target features"). WP-630.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ stdout: crc 93ad1061 popcnt 17 aes 1111 pclmul 0a csr 1 0
#include <immintrin.h>
#include <cstdio>

__attribute__((target("sse4.2"))) static unsigned crc(unsigned c, unsigned char v) {
  return _mm_crc32_u8(c, v);
}

__attribute__((target("popcnt"))) static int pop(unsigned v) { return _mm_popcnt_u32(v); }

__attribute__((target("aes"))) static int aes() {
  __m128i k = _mm_set1_epi8(0x2b);
  __m128i d = _mm_aesenc_si128(_mm_set1_epi32(0x01234567), k);
  d = _mm_aesdeclast_si128(d, _mm_aeskeygenassist_si128(k, 1));
  return _mm_movemask_epi8(d);
}

__attribute__((target("pclmul"))) static int clmul() {
  __m128i p = _mm_clmulepi64_si128(_mm_cvtsi32_si128(3), _mm_cvtsi32_si128(6), 0x00);
  return _mm_cvtsi128_si32(p);
}

int main() {
  static char line[64];
  _mm_pause();
  _mm_mfence();
  _mm_lfence();
  _mm_sfence();
  _mm_clflush(line);
  unsigned csr = _mm_getcsr();
  _mm_setcsr(csr | _MM_FLUSH_ZERO_ON);
  unsigned fz = (_mm_getcsr() & _MM_FLUSH_ZERO_MASK) != 0;
  _mm_setcsr(csr);
  unsigned back = (_mm_getcsr() & _MM_FLUSH_ZERO_MASK) != 0;
  std::printf("crc %08x popcnt %d aes %04x pclmul %02x csr %u %u\n", crc(0, 'a'),
              pop(0xf0f0f0f1u), aes() & 0xffff, clmul(), fz, back);
}
