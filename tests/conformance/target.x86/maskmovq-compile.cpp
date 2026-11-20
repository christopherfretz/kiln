// [driver] clang 19's MMX `__builtin_ia32_maskmovq` (`_mm_maskmove_si64`
// in kiln's `<xmmintrin.h>`) is emitted as clang 20+'s header defines the
// intrinsic: `maskmovdqu` of the operands widened to 128 bits, the
// pointer backed up by 8 bytes and both operands shifted left by 8 bytes
// when it lies 8 to 15 bytes before a page boundary (LLVM 21 has no
// `x86_mmx`, so clang 19's IR cannot be the reference; clang 22 has no
// such builtin). It used to be "not supported yet". Pinned to x86-64, no
// headers. WP-1739.
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu -msse2
typedef char v8qi __attribute__((__vector_size__(8)));
void mask_store(v8qi d, v8qi n, char *p) { __builtin_ia32_maskmovq(d, n, p); }
