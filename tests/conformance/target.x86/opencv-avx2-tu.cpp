// [driver] Reduced from an OpenCV 4.12.0 `modules/core/*.avx2.cpp`
// dispatch TU (the x86_64 sweep): one TU under `-mavx2 -mfma -mf16c`
// calling `_mm256_extracti128_si256`, `_mm256_permute2x128_si256`,
// `_mm256_cvtph_ps` and `_mm256_fmadd_pd` / `_ps`, whose builtins clang
// lowers to generic IR (`EmitX86BuiltinExpr`), compiles and links
// (spec/landed/codegen/WP-630.md "Generic x86 lowering"). Before WP-1242
// each was "code generation for __builtin_ia32_... is not supported yet".
// Host-restricted (no pinned `--target`). WP-1242.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++17
//@ args: -O2 -mavx2 -mfma -mf16c
//@ stdout: 36 36 3 36 20.4
#include <immintrin.h>
#include <cstdio>

static inline int v_reduce_sum(__m256i a) {
  __m128i s = _mm_add_epi32(_mm256_castsi256_si128(a), _mm256_extracti128_si256(a, 1));
  s = _mm_hadd_epi32(s, s);
  s = _mm_hadd_epi32(s, s);
  return _mm_cvtsi128_si32(s);
}
static inline __m256i v_combine_high(__m256i a, __m256i b) {
  return _mm256_permute2x128_si256(a, b, 0x31);
}
static void cvt_f16f32(const unsigned short *src, float *dst, int n) {
  for (int i = 0; i + 8 <= n; i += 8)
    _mm256_storeu_ps(dst + i, _mm256_cvtph_ps(_mm_loadu_si128(reinterpret_cast<const __m128i *>(src + i))));
}
static double dot_f64(const double *a, const double *b, int n) {
  __m256d s = _mm256_setzero_pd();
  for (int i = 0; i + 4 <= n; i += 4)
    s = _mm256_fmadd_pd(_mm256_loadu_pd(a + i), _mm256_loadu_pd(b + i), s);
  alignas(32) double t[4];
  _mm256_store_pd(t, s);
  return t[0] + t[1] + t[2] + t[3];
}
static float dot_f32(const float *a, const float *b, int n) {
  __m256 s = _mm256_setzero_ps();
  for (int i = 0; i + 8 <= n; i += 8)
    s = _mm256_fmadd_ps(_mm256_loadu_ps(a + i), _mm256_loadu_ps(b + i), s);
  alignas(32) float t[8];
  _mm256_store_ps(t, s);
  return t[0] + t[1] + t[2] + t[3] + t[4] + t[5] + t[6] + t[7];
}

int main() {
  __m256i a = _mm256_setr_epi32(1, 2, 3, 4, 5, 6, 7, 8);
  __m256i b = _mm256_setr_epi32(0, 0, 0, 0, 1, 2, 3, 4);
  unsigned short h[8] = {0x3c00, 0x4000, 0x3800, 0x3c00, 0x3c00, 0x3c00, 0x3c00, 0x3c00};
  float f[8];
  cvt_f16f32(h, f, 8);
  double da[4] = {1, 2, 3, 4}, db[4] = {1, 1, 1, 1};
  float fa[8] = {1, 1, 1, 1, 1, 1, 1, 1};
  std::printf("%d %d %g %g %g\n", v_reduce_sum(a), v_reduce_sum(v_combine_high(a, b)),
              (double)(f[0] + f[1]), dot_f64(da, db, 4) * 3.6, (double)dot_f32(f, fa, 8) * 2.4);
}
