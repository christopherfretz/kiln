// [driver] The AVX2 / FMA / F16C intrinsics clang lowers to generic IR
// (`EmitX86BuiltinExpr`) compute the documented lanes:
// `_mm256_extracti128_si256` the 128-bit half the immediate names,
// `_mm256_permute2x128_si256` the halves of `a:b` (or zero, bit 3) each
// nibble selects, `_mm256_cvtph_ps` the IEEE half values, and
// `_mm256_fmadd_pd` / `_ps` `a * b + c` rounded once
// (spec/landed/codegen/WP-630.md "Generic x86 lowering"). Host-restricted
// (no pinned `--target`): it runs on the x86_64 lane. WP-1242.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ args: -mavx2 -mfma -mf16c
//@ stdout-file: avx2-fma-f16c-run.stdout
#include <immintrin.h>
#include <cstdio>

static void pi(const char *n, __m128i v) {
  alignas(16) int x[4];
  _mm_store_si128(reinterpret_cast<__m128i *>(x), v);
  std::printf("%s: %d %d %d %d\n", n, x[0], x[1], x[2], x[3]);
}
static void pi(const char *n, __m256i v) {
  alignas(32) int x[8];
  _mm256_store_si256(reinterpret_cast<__m256i *>(x), v);
  std::printf("%s: %d %d %d %d %d %d %d %d\n", n, x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]);
}
static void pf(const char *n, __m256 v) {
  alignas(32) float x[8];
  _mm256_store_ps(x, v);
  std::printf("%s: %g %g %g %g %g %g %g %g\n", n, x[0], x[1], x[2], x[3], x[4], x[5], x[6], x[7]);
}
static void pd(const char *n, __m256d v) {
  alignas(32) double x[4];
  _mm256_store_pd(x, v);
  std::printf("%s: %.17g %.17g %.17g %.17g\n", n, x[0], x[1], x[2], x[3]);
}

int main() {
  __m256i a = _mm256_setr_epi32(1, 2, 3, 4, 5, 6, 7, 8);
  __m256i b = _mm256_setr_epi32(-1, -2, -3, -4, -5, -6, -7, -8);
  pi("extracti128 0", _mm256_extracti128_si256(a, 0));
  pi("extracti128 1", _mm256_extracti128_si256(a, 1));
  pi("permute2x128 0x20", _mm256_permute2x128_si256(a, b, 0x20));
  pi("permute2x128 0x31", _mm256_permute2x128_si256(a, b, 0x31));
  pi("permute2x128 0x12", _mm256_permute2x128_si256(a, b, 0x12));
  pi("permute2x128 0x83", _mm256_permute2x128_si256(a, b, 0x83));
  // 1.0, -2.0, 0.5, 65504 (max), 2^-24 (min subnormal), +inf, -0.0, 3.140625
  __m128i h = _mm_setr_epi16(0x3c00, (short)0xc000, 0x3800, 0x7bff, 0x0001, 0x7c00,
                             (short)0x8000, 0x4248);
  pf("cvtph_ps", _mm256_cvtph_ps(h));
  __m256 x = _mm256_setr_ps(1, 2, 3, 4, 0.1f, -1.5f, 1e30f, 1.00000012f);
  __m256 y = _mm256_setr_ps(5, 6, 7, 8, 10, 2, 1e10f, 0.99999988f);
  __m256 z = _mm256_setr_ps(0.5f, -12, 0, 1, -1, 3, -3e38f, -1);
  pf("fmadd_ps", _mm256_fmadd_ps(x, y, z));
  // One rounding: (1 + 2^-52) * (1 - 2^-52) - 1 is -2^-104, not 0.
  __m256d u = _mm256_setr_pd(1 + 0x1p-52, 3, 0.1, -2);
  __m256d v = _mm256_setr_pd(1 - 0x1p-52, 0.25, 10, 8);
  __m256d w = _mm256_setr_pd(-1, 1, -1, 0.5);
  pd("fmadd_pd", _mm256_fmadd_pd(u, v, w));
}
