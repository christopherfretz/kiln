// [clang ext] Function multiversioning on x86-64: `target_clones` emits
// one clone per listed target plus an ifunc resolver that picks one from
// the running CPU's features. With the builtin <immintrin.h> the avx2
// clone can call AVX2 intrinsics (spec/landed/codegen/WP-810.md: kiln
// supports AArch64 multiversioning only; WP-1000).
//@ args: --target=x86_64-unknown-linux-gnu -ffreestanding
//@ expect: compile-ok
//@ xfail(kiln): WP-1000 follow-up (WP-1044), x86-64 function multiversioning (target_clones resolvers) is not implemented
#include <immintrin.h>
__attribute__((target_clones("avx2", "default"))) int sum(const int *p) {
#ifdef __AVX2__
  __m256i v = _mm256_loadu_si256((const __m256i *)p);
  v = _mm256_hadd_epi32(v, v);
  return _mm256_extract_epi32(v, 0);
#else
  return p[0] + p[1];
#endif
}
int call(const int *p) { return sum(p); }
