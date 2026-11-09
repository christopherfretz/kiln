// clang's x86 feature flags `-m<feature>` / `-mno-<feature>`
// (`m_x86_Features_Group`: `-msse4.2`, `-mavx2`, `-mpclmul`, `-msha`, ...)
// add or remove one target feature on top of `-march`, with its
// `__<FEATURE>__` macro (spec/driver.md "x86-64 target features",
// WP-1000). Before WP-1000 every such flag was an unknown argument, so
// CMake's CPU-dispatch probes failed and the SIMD paths were compiled out
// (x86_64 sweep: OpenCV and Bitcoin Core).
//@ args: --target=x86_64-unknown-linux-gnu -msse4.2 -mavx2 -mpclmul -msha -mno-avx512f
//@ expect: compile-ok
#if !defined(__SSE4_2__) || !defined(__AVX2__) || !defined(__PCLMUL__) || !defined(__SHA__)
#error "the -m<feature> flags did not enable their features"
#endif
#ifdef __AVX512F__
#error "-mno-avx512f did not disable avx512f"
#endif
// sse4.2's CRC32, callable only with the feature enabled.
unsigned long long crc(unsigned long long c, unsigned long long v) {
  return __builtin_ia32_crc32di(c, v);
}
