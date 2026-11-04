// [driver] The SSE to SSE4.2, POPCNT and CRC32 intrinsics under
// -march=westmere, each through the builtin clang lowers it with: 1:1
// `llvm.x86.*` intrinsics (`pmaddwd`, `packsswb`, `pshufb`, `phaddd`,
// `ptestz`, `pcmpistri`, `crc32`, ...) and the generic lowerings of
// `EmitX86BuiltinExpr` (`pshufd`/`pshuflw`/`pshufhw`, `shufps`/`shufpd`,
// `pslldqi`/`psrldqi`, `palignr`, `blendps`/`pblendw`, `pmuludq`/`pmuldq`,
// `vec_ext`/`vec_set`, `cmp<cc>ps`/`ss`, `sqrtps`/`ss`)
// (spec/landed/codegen/WP-630.md "Custom lowering"). WP-630.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ args: -march=westmere
//@ stdout-file: sse-westmere-run.stdout
#include <nmmintrin.h>
#include <cstdio>

static void p8(const char *n, __m128i v) {
  alignas(16) unsigned char b[16];
  _mm_store_si128(reinterpret_cast<__m128i *>(b), v);
  std::printf("%s:", n);
  for (unsigned char c : b)
    std::printf(" %02x", c);
  std::printf("\n");
}

static void pf(const char *n, __m128 v) {
  alignas(16) float f[4];
  _mm_store_ps(f, v);
  std::printf("%s: %g %g %g %g\n", n, f[0], f[1], f[2], f[3]);
}

static void pd(const char *n, __m128d v) {
  alignas(16) double d[2];
  _mm_store_pd(d, v);
  std::printf("%s: %g %g\n", n, d[0], d[1]);
}

int main() {
  __m128i a = _mm_setr_epi8(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15);
  __m128i b = _mm_setr_epi8(-1, -2, -3, -4, 50, 60, 70, 80, 90, 100, 110, 120, -128, 127, 0, 1);
  p8("shuffle_epi32", _mm_shuffle_epi32(a, _MM_SHUFFLE(0, 1, 2, 3)));
  p8("shufflelo", _mm_shufflelo_epi16(a, _MM_SHUFFLE(0, 0, 3, 2)));
  p8("shufflehi", _mm_shufflehi_epi16(a, _MM_SHUFFLE(1, 3, 0, 2)));
  p8("slli_si128", _mm_slli_si128(a, 3));
  p8("srli_si128", _mm_srli_si128(a, 5));
  p8("srli_si128 16", _mm_srli_si128(a, 16));
  p8("alignr 4", _mm_alignr_epi8(a, b, 4));
  p8("alignr 20", _mm_alignr_epi8(a, b, 20));
  p8("blend_epi16", _mm_blend_epi16(a, b, 0x5a));
  p8("blendv_epi8", _mm_blendv_epi8(a, b, _mm_setr_epi8(0, -1, 0, -1, 0, 0, -1, -1, 0, 0, 0, 0, -1, 0, -1, 0)));
  p8("mul_epu32", _mm_mul_epu32(b, b));
  p8("mul_epi32", _mm_mul_epi32(b, a));
  p8("madd_epi16", _mm_madd_epi16(a, b));
  p8("packs_epi16", _mm_packs_epi16(b, a));
  p8("packus_epi32", _mm_packus_epi32(b, a));
  p8("shuffle_epi8", _mm_shuffle_epi8(a, b));
  p8("hadd_epi32", _mm_hadd_epi32(a, b));
  p8("abs_epi8", _mm_abs_epi8(b));
  p8("sign_epi8", _mm_sign_epi8(a, b));
  p8("min_epu8", _mm_min_epu8(a, b));
  p8("max_epi8", _mm_max_epi8(a, b));
  p8("cvtepi8_epi32", _mm_cvtepi8_epi32(b));
  p8("sad_epu8", _mm_sad_epu8(a, b));
  p8("sll_epi32", _mm_sll_epi32(a, _mm_cvtsi32_si128(3)));
  p8("srai_epi16", _mm_srai_epi16(b, 4));
  p8("insert_epi8", _mm_insert_epi8(a, 0x77, 9));
  p8("insert_epi32", _mm_insert_epi32(a, 0x12345678, 2));
  p8("insert_epi16", _mm_insert_epi16(a, 0x4321, 6));
  p8("cmpgt_epi64", _mm_cmpgt_epi64(b, a));
  p8("minpos", _mm_minpos_epu16(b));
  p8("mpsadbw", _mm_mpsadbw_epu8(a, b, 5));
  std::printf("extract: %d %d %d %lld\n", _mm_extract_epi8(b, 13), _mm_extract_epi16(b, 6),
              _mm_extract_epi32(b, 1), _mm_extract_epi64(b, 1));
  std::printf("movemask: %d test: %d %d %d\n", _mm_movemask_epi8(b), _mm_testz_si128(a, b),
              _mm_testc_si128(a, a), _mm_testnzc_si128(a, b));
  __m128i s = _mm_setr_epi8('h', 'e', 'l', 'l', 'o', ' ', 'w', 'o', 'r', 'l', 'd', 0, 0, 0, 0, 0);
  __m128i w = _mm_setr_epi8('w', 'o', 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
  std::printf("cmpistri: %d %d cmpestri: %d\n", _mm_cmpistri(w, s, _SIDD_CMP_EQUAL_ORDERED),
              _mm_cmpistrz(w, s, _SIDD_CMP_EQUAL_ORDERED),
              _mm_cmpestri(w, 2, s, 11, _SIDD_CMP_EQUAL_ORDERED));
  p8("cmpistrm", _mm_cmpistrm(w, s, _SIDD_CMP_EQUAL_ANY | _SIDD_UNIT_MASK));
  std::printf("crc32: %08x %08x %08x %016llx\n", _mm_crc32_u8(0, 'a'), _mm_crc32_u16(1, 0x1234),
              _mm_crc32_u32(~0u, 0xdeadbeef), (unsigned long long)_mm_crc32_u64(7, 0x0123456789abcdefull));
  std::printf("popcnt: %d %lld\n", _mm_popcnt_u32(0xf0f0f0f1u), _mm_popcnt_u64(~0ull));
  __m128 x = _mm_setr_ps(1.0f, 4.0f, -2.0f, 9.0f);
  __m128 y = _mm_setr_ps(2.0f, 4.0f, -3.0f, __builtin_nanf(""));
  pf("shuffle_ps", _mm_shuffle_ps(x, y, _MM_SHUFFLE(3, 0, 2, 1)));
  pf("blend_ps", _mm_blend_ps(x, y, 0x6));
  pf("sqrt_ps", _mm_sqrt_ps(_mm_setr_ps(4, 9, 16, 25)));
  pf("sqrt_ss", _mm_sqrt_ss(_mm_setr_ps(36, 9, 16, 25)));
  pf("dp_ps", _mm_dp_ps(x, y, 0xf1));
  pf("round_ps", _mm_round_ps(_mm_setr_ps(1.5f, 2.5f, -1.5f, 0.4f), _MM_FROUND_TO_NEAREST_INT));
  pf("hadd_ps", _mm_hadd_ps(x, y));
  pf("insert_ps", _mm_insert_ps(x, y, 0x90));
  std::printf("extract_ps: %d movemask_ps: %d\n", _mm_extract_ps(x, 2), _mm_movemask_ps(_mm_cmplt_ps(x, y)));
  p8("cmplt_ps", _mm_castps_si128(_mm_cmplt_ps(x, y)));
  p8("cmpnle_ps", _mm_castps_si128(_mm_cmpnle_ps(x, y)));
  p8("cmpunord_ps", _mm_castps_si128(_mm_cmpunord_ps(x, y)));
  p8("cmpeq_ss", _mm_castps_si128(_mm_cmpeq_ss(x, y)));
  p8("cmpge_ss", _mm_castps_si128(_mm_cmpge_ss(y, x)));
  std::printf("comi: %d %d ucomi: %d\n", _mm_comilt_ss(x, y), _mm_comieq_ss(x, y), _mm_ucomigt_ss(y, x));
  __m128d u = _mm_setr_pd(1.5, -2.25);
  __m128d v = _mm_setr_pd(-3.0, 8.0);
  pd("shuffle_pd", _mm_shuffle_pd(u, v, 1));
  pd("blend_pd", _mm_blend_pd(u, v, 2));
  pd("sqrt_sd", _mm_sqrt_sd(u, _mm_setr_pd(81.0, 4.0)));
  pd("sqrt_pd", _mm_sqrt_pd(_mm_setr_pd(2.25, 100.0)));
  pd("cmple_pd", _mm_cmple_pd(u, v));
  pd("addsub_pd", _mm_addsub_pd(u, v));
  pd("round_sd", _mm_round_sd(u, v, _MM_FROUND_TO_NEG_INF));
  pd("cvtepi32_pd", _mm_cvtepi32_pd(_mm_setr_epi32(-7, 11, 0, 0)));
  std::printf("cvt: %d %d %lld\n", _mm_cvtsd_si32(u), _mm_cvttsd_si32(v), _mm_cvtss_si64(x));
  return 0;
}
