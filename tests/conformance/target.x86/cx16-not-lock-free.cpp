// [atomics.lockfree] The default `x86-64` CPU has no `cx16`: 16-byte
// atomics are not lock-free (clang `X86TargetInfo::setMaxAtomicWidth`).
// WP-630.
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
static_assert(!__atomic_always_lock_free(16, 0));
static_assert(__atomic_always_lock_free(8, 0));
#ifdef __GCC_HAVE_SYNC_COMPARE_AND_SWAP_16
#error "cx16"
#endif
