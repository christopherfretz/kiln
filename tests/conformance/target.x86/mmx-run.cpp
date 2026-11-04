// [driver] clang 19's MMX builtins (`<mmintrin.h>`, the `__m64` parts of
// `<xmmintrin.h>`, `<emmintrin.h>` and `<tmmintrin.h>`): LLVM 21 has no
// `x86_mmx`, so each is emitted as clang 20+'s headers define the
// intrinsic, on 64-bit vectors or on the SSE2 / SSSE3 builtin of the
// widened operands (spec/landed/codegen/WP-630.md "MMX"). clang 19's
// `_mm_extract_pi16` sign-extends a negative lane (clang 20 added an
// `(unsigned short)` cast), so a positive lane is read. WP-630.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ args: -march=westmere
//@ stdout-file: mmx-run.stdout
#include <tmmintrin.h>
#include <cstdio>

static void p(const char *n, __m64 v) {
  unsigned long long x = (unsigned long long)_mm_cvtm64_si64(v);
  std::printf("%s: %016llx\n", n, x);
}

int main() {
  __m64 a = _mm_set_pi8(8, 7, 6, 5, 4, 3, 2, 1);
  __m64 b = _mm_set_pi8(-100, 100, -3, 3, 127, -128, 50, -1);
  __m64 w = _mm_set_pi16(-30000, 1000, -2, 3);
  __m64 d = _mm_set_pi32(-5, 70000);
  p("add_pi8", _mm_add_pi8(a, b));
  p("adds_pi8", _mm_adds_pi8(a, b));
  p("adds_pu8", _mm_adds_pu8(a, b));
  p("subs_pi16", _mm_subs_pi16(w, _mm_set1_pi16(10000)));
  p("subs_pu16", _mm_subs_pu16(w, _mm_set1_pi16(5)));
  p("add_si64", _mm_add_si64(a, b));
  p("mullo_pi16", _mm_mullo_pi16(w, w));
  p("mulhi_pi16", _mm_mulhi_pi16(w, w));
  p("mulhi_pu16", _mm_mulhi_pu16(w, w));
  p("madd_pi16", _mm_madd_pi16(w, w));
  p("mul_su32", _mm_mul_su32(d, d));
  p("andnot", _mm_andnot_si64(a, b));
  p("xor", _mm_xor_si64(a, b));
  p("cmpgt_pi8", _mm_cmpgt_pi8(a, b));
  p("cmpeq_pi16", _mm_cmpeq_pi16(w, _mm_set_pi16(0, 1000, 0, 3)));
  p("unpackhi_pi8", _mm_unpackhi_pi8(a, b));
  p("unpacklo_pi16", _mm_unpacklo_pi16(w, a));
  p("unpackhi_pi32", _mm_unpackhi_pi32(d, a));
  p("packs_pi16", _mm_packs_pi16(w, a));
  p("packs_pi32", _mm_packs_pi32(d, d));
  p("packs_pu16", _mm_packs_pu16(w, a));
  p("sll_pi16", _mm_sll_pi16(w, _mm_cvtsi32_si64(3)));
  p("slli_pi32", _mm_slli_pi32(d, 4));
  p("srai_pi16", _mm_srai_pi16(w, 2));
  p("srl_si64", _mm_srl_si64(b, _mm_cvtsi32_si64(12)));
  p("max_pi16", _mm_max_pi16(w, _mm_set1_pi16(0)));
  p("min_pu8", _mm_min_pu8(a, b));
  p("avg_pu8", _mm_avg_pu8(a, b));
  p("sad_pu8", _mm_sad_pu8(a, b));
  p("shuffle_pi16", _mm_shuffle_pi16(w, _MM_SHUFFLE(0, 1, 2, 3)));
  std::printf("movemask_pi8: %d extract_pi16: %d cvtsi64_si32: %d\n", _mm_movemask_pi8(b),
              _mm_extract_pi16(w, 2), _mm_cvtsi64_si32(d));
  p("insert_pi16", _mm_insert_pi16(w, 77, 1));
  p("abs_pi8", _mm_abs_pi8(b));
  p("sign_pi16", _mm_sign_pi16(w, _mm_set_pi16(-1, 0, 1, -1)));
  p("hadd_pi16", _mm_hadd_pi16(w, a));
  p("hsubs_pi16", _mm_hsubs_pi16(w, a));
  p("maddubs_pi16", _mm_maddubs_pi16(a, b));
  p("mulhrs_pi16", _mm_mulhrs_pi16(w, w));
  p("shuffle_pi8", _mm_shuffle_pi8(a, _mm_set_pi8(0, 1, 2, 3, -1, 5, 6, 7)));
  p("alignr_pi8", _mm_alignr_pi8(a, b, 3));
  __m128 f = _mm_cvtpi32_ps(_mm_set_ps(4, 3, 2, 1), d);
  __m128d g = _mm_cvtpi32_pd(d);
  std::printf("cvtpi32_ps: %g %g %g %g cvtpi32_pd: %g %g\n", f[0], f[1], f[2], f[3], g[0], g[1]);
  p("cvtps_pi32", _mm_cvtps_pi32(_mm_set_ps(9, 9, -2.5f, 3.5f)));
  p("cvttps_pi32", _mm_cvttps_pi32(_mm_set_ps(9, 9, -2.5f, 3.5f)));
  p("cvtpd_pi32", _mm_cvtpd_pi32(_mm_set_pd(-7.5, 2.5)));
  p("cvttpd_pi32", _mm_cvttpd_pi32(_mm_set_pd(-7.5, 2.5)));
  long long out = 0;
  _mm_stream_pi(reinterpret_cast<__m64 *>(&out), a);
  _mm_empty();
  std::printf("stream: %llx\n", out);
}
