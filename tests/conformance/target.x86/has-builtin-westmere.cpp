// [cpp.cond] Under -march=westmere the CPU's features (clang 21's
// `"target-features"` for it) answer `__has_builtin`, and its predefined
// macros are clang's (spec/landed/driver/WP-630.md). WP-630.
//@ args: --target=x86_64-unknown-linux-gnu -march=westmere
//@ strict-diags
static_assert(__has_builtin(__builtin_ia32_crc32qi));
static_assert(__has_builtin(__builtin_ia32_pcmpistri128));
static_assert(__has_builtin(__builtin_ia32_pclmulqdq128));
static_assert(!__has_builtin(__builtin_ia32_aesenc128));
static_assert(!__has_builtin(__builtin_ia32_pshufb256));
#if !defined(__SSE4_2__) || !defined(__SSE4_1__) || !defined(__SSSE3__) || !defined(__SSE3__)
#error "SSE predefines"
#endif
#if !defined(__POPCNT__) || !defined(__CRC32__) || !defined(__PCLMUL__) || !defined(__LAHF_SAHF__)
#error "feature predefines"
#endif
#if !defined(__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16) || !defined(__corei7__) || defined(__k8__)
#error "CPU predefines"
#endif
#if defined(__AES__) || defined(__AVX__)
#error "westmere has neither"
#endif
