// [driver] The x86 intrinsic headers under -march=westmere (its
// predefines enable the SSE3 to SSE4.2, POPCNT, CRC32 and PCLMUL parts):
// every inline function type-checks, and the macro intrinsics that pass
// an immediate to a builtin (`_mm_shuffle_epi32`, `_mm_blend_epi16`,
// `_mm_cmpistri`, ...) accept integer constant expressions
// (spec/landed/sema/WP-630.md "Immediates"). WP-630.
//@ args: --target=x86_64-unknown-linux-gnu -std=gnu++23 -march=westmere -Wall -Werror
//@ std: c++23
//@ strict-diags
#include <immintrin.h>
#include <x86intrin.h>

#if !defined(__SSE4_2__) || !defined(__POPCNT__) || !defined(__PCLMUL__) || defined(__AES__)
#error "westmere predefines"
#endif

constexpr int imm = _MM_SHUFFLE(3, 2, 1, 0);

__m128i f(__m128i a, __m128i b, __m128 x, __m128d y) {
  a = _mm_shuffle_epi32(a, imm);
  a = _mm_shufflelo_epi16(a, 0x1b);
  a = _mm_shufflehi_epi16(a, 0x1b);
  a = _mm_slli_si128(a, 3);
  a = _mm_srli_si128(a, 5);
  a = _mm_alignr_epi8(a, b, 7);
  a = _mm_blend_epi16(a, b, 0xaa);
  a = _mm_insert_epi8(a, 1, 3);
  a = _mm_insert_epi32(a, 1, 2);
  a = _mm_insert_epi64(a, 1, 1);
  a = _mm_insert_epi16(a, 1, 4);
  a = _mm_mpsadbw_epu8(a, b, 5);
  a = _mm_clmulepi64_si128(a, b, 0x11);
  x = _mm_shuffle_ps(x, x, imm);
  x = _mm_blend_ps(x, x, 5);
  x = _mm_dp_ps(x, x, 0xff);
  x = _mm_round_ps(x, _MM_FROUND_TO_NEAREST_INT);
  x = _mm_insert_ps(x, x, 0x10);
  y = _mm_shuffle_pd(y, y, 1);
  y = _mm_blend_pd(y, y, 1);
  y = _mm_cmp_pd(y, y, _CMP_LT_OS);
  int i = _mm_extract_epi8(a, 15) + _mm_extract_epi16(a, 7) + _mm_extract_epi32(a, 3) +
          _mm_extract_ps(x, 2) + _mm_cmpistri(a, b, _SIDD_CMP_EQUAL_ORDERED) +
          _mm_cmpestri(a, 4, b, 8, _SIDD_UBYTE_OPS);
  long long l = _mm_extract_epi64(a, 1);
  return _mm_add_epi32(a, _mm_set1_epi32(i + (int)l + (int)y[0]));
}
