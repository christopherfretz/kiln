// [cpp.cond] `__has_builtin` of an x86 builtin answers whether its
// required target features hold for the command line, as clang's
// `evaluateRequiredTargetFeatures` (spec/pp.md, spec/landed/sema/WP-630.md
// "`__has_builtin`"): the SSE2 ones under the default `x86-64`, the
// SSE4.2 ones only under a CPU that has them. WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
static_assert(__has_builtin(__builtin_ia32_pmovmskb128));
static_assert(__has_builtin(__builtin_ia32_pause));
static_assert(__has_builtin(__builtin_ia32_rdtsc));
static_assert(!__has_builtin(__builtin_ia32_crc32qi));
static_assert(!__has_builtin(__builtin_ia32_pcmpistri128));
static_assert(!__has_builtin(__builtin_ia32_aesenc128));
static_assert(!__has_builtin(__builtin_ia32_vpdpbusd128));
#if defined(__SSE3__) || defined(__POPCNT__) || !defined(__SSE2__)
#error "x86-64 predefines"
#endif
