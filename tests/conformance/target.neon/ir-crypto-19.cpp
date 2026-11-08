// [driver] The cryptographic-extension NEON builtins emit clang 19's IR:
// one call of the non-overloaded `llvm.aarch64.crypto.*` intrinsic on
// `<16 x i8>`, `<4 x i32>` and `i32` operands, the AES builtins' trailing
// type code dropped (spec/codegen.md "AArch64 NEON builtins"). WP-811.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8-a+crypto -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto f_aese(uint8x16_t a, uint8x16_t b) { return vaeseq_u8(a, b); }
auto f_aesd(uint8x16_t a, uint8x16_t b) { return vaesdq_u8(a, b); }
auto f_aesmc(uint8x16_t a) { return vaesmcq_u8(a); }
auto f_aesimc(uint8x16_t a) { return vaesimcq_u8(a); }
auto f_sha1c(uint32x4_t a, uint32_t e, uint32x4_t w) { return vsha1cq_u32(a, e, w); }
auto f_sha1m(uint32x4_t a, uint32_t e, uint32x4_t w) { return vsha1mq_u32(a, e, w); }
auto f_sha1p(uint32x4_t a, uint32_t e, uint32x4_t w) { return vsha1pq_u32(a, e, w); }
auto f_sha1h(uint32_t e) { return vsha1h_u32(e); }
auto f_sha1su0(uint32x4_t a, uint32x4_t b, uint32x4_t c) { return vsha1su0q_u32(a, b, c); }
auto f_sha1su1(uint32x4_t a, uint32x4_t b) { return vsha1su1q_u32(a, b); }
auto f_sha256h(uint32x4_t a, uint32x4_t b, uint32x4_t c) { return vsha256hq_u32(a, b, c); }
auto f_sha256h2(uint32x4_t a, uint32x4_t b, uint32x4_t c) { return vsha256h2q_u32(a, b, c); }
auto f_sha256su0(uint32x4_t a, uint32x4_t b) { return vsha256su0q_u32(a, b); }
auto f_sha256su1(uint32x4_t a, uint32x4_t b, uint32x4_t c) { return vsha256su1q_u32(a, b, c); }
