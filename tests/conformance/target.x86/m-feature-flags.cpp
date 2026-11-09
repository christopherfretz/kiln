// clang's x86 feature flags `-m<feature>` / `-mno-<feature>`
// (`m_x86_Features_Group`: `-msse4.2`, `-mavx2`, `-mpclmul`, `-msha`, ...)
// add or remove one target feature on top of `-march`, with its
// `__<FEATURE>__` macro. kiln knows only `-march=<cpu>`, so every such
// flag is an unknown argument: CMake's CPU-dispatch probes fail and the
// SIMD paths are compiled out (x86_64 sweep: OpenCV built 473 of clang's
// 537 TUs, SSE3 to AVX2 "not supported by C++ compiler"; Bitcoin Core
// lost its SSE4.1, AVX2, SHA-NI, SSE4.2 and CLMUL variants).
//@ args: --target=x86_64-unknown-linux-gnu -msse4.2 -mavx2 -mpclmul -msha -mno-avx512f
//@ expect: compile-ok
//@ xfail(kiln): WP-961 follow-up, x86 -m<feature>/-mno-<feature> flags are unknown arguments (seen building opencv and bitcoin_core on x86_64)
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
