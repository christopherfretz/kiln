// [driver] The streaming intrinsics of <emmintrin.h> / <smmintrin.h>:
// `_mm_stream_si128` / `_mm_stream_pd` are `__builtin_nontemporal_store`,
// `_mm_stream_load_si128` is `__builtin_nontemporal_load` (a store or load
// through the pointer with its known alignment, `!nontemporal`), and
// `_mm_stream_si32` / `_mm_stream_si64` are `__builtin_ia32_movnti` (an
// unaligned nontemporal store) (spec/landed/sema/WP-630.md "Generic
// builtins"). WP-630.
//@ requires: target-x86_64
//@ expect: run
//@ std: c++23
//@ args: -march=westmere
//@ stdout: 3 4 1 2 -7 9 17
#include <smmintrin.h>
#include <cstdio>

int main() {
  alignas(16) int a[4];
  alignas(16) double d[2];
  int i32 = 0;
  long long i64 = 0;
  _mm_stream_si128(reinterpret_cast<__m128i *>(a), _mm_setr_epi32(1, 2, 3, 4));
  _mm_stream_pd(d, _mm_setr_pd(-7.0, 9.0));
  _mm_stream_si32(&i32, 8);
  _mm_stream_si64(&i64, 9);
  _mm_sfence();
  __m128i v = _mm_stream_load_si128(reinterpret_cast<__m128i *>(a));
  v = _mm_shuffle_epi32(v, _MM_SHUFFLE(1, 0, 3, 2));
  std::printf("%d %d %d %d %g %g %lld\n", _mm_extract_epi32(v, 0), _mm_extract_epi32(v, 1),
              _mm_extract_epi32(v, 2), _mm_extract_epi32(v, 3), d[0], d[1], i32 + i64);
}
