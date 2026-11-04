// [atomics.lockfree] A CPU with `cx16` (-march=westmere) makes 16-byte
// atomics lock-free: clang's `X86TargetInfo::setMaxAtomicWidth` raises the
// max inline atomic width to 128 bits, so `__atomic_always_lock_free(16)`
// is true and `__GCC_HAVE_SYNC_COMPARE_AND_SWAP_16` is defined
// (spec/landed/driver/WP-630.md). WP-630.
//@ args: --target=x86_64-unknown-linux-gnu -march=westmere
//@ strict-diags
static_assert(__atomic_always_lock_free(16, 0));
#ifndef __GCC_HAVE_SYNC_COMPARE_AND_SWAP_16
#error "cx16"
#endif
