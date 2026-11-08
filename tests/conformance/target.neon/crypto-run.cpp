// [driver] The cryptographic extension under `-march=armv8-a+crypto`: the
// AES rounds `vaes{e,d,mc,imc}q_u8`, the SHA-1 steps `vsha1{c,m,p}q_u32`,
// `vsha1h_u32`, `vsha1su{0,1}q_u32` and the SHA-256 steps
// `vsha256h[2]q_u32`, `vsha256su{0,1}q_u32` each lower to the
// `llvm.aarch64.crypto.*` intrinsic of that name; the printed lane hashes
// are clang 19's (spec/codegen.md "AArch64 NEON builtins"). WP-811 (Bitcoin
// Core's ARM SHA-NI configure probe).
//@ expect: run
//@ requires: target-aarch64
//@ args: -march=armv8-a+crypto
//@ strict-diags
//@ stdout: 53b9a3dc 4450eafc 360ae50d 5c3be757 | 131ee03a b32b854d f23cbf97 f7ab6fbb 856b4848 d20d1a18 | 6eb4f267 903363af ba43845e c5661c90
extern "C" int printf(const char *, ...);
#include <arm_neon.h>

static unsigned sum(uint32x4_t v) {
  return vgetq_lane_u32(v, 0) ^ (vgetq_lane_u32(v, 1) * 3) ^
         (vgetq_lane_u32(v, 2) * 5) ^ (vgetq_lane_u32(v, 3) * 7);
}
static unsigned sum8(uint8x16_t v) { return sum(vreinterpretq_u32_u8(v)); }

int main() {
  uint32_t ka[4] = {0x6a09e667u, 0xbb67ae85u, 0x3c6ef372u, 0xa54ff53au};
  uint32_t kb[4] = {0x510e527fu, 0x9b05688cu, 0x1f83d9abu, 0x5be0cd19u};
  uint32_t kc[4] = {0x428a2f98u, 0x71374491u, 0xb5c0fbcfu, 0xe9b5dba5u};
  uint32x4_t a = vld1q_u32(ka), b = vld1q_u32(kb), c = vld1q_u32(kc);
  printf("%08x %08x %08x %08x | ", sum(vsha256hq_u32(a, b, c)),
         sum(vsha256h2q_u32(a, b, c)), sum(vsha256su0q_u32(a, b)),
         sum(vsha256su1q_u32(a, b, c)));
  printf("%08x %08x %08x %08x %08x %08x | ", sum(vsha1cq_u32(a, 0x12345678u, b)),
         sum(vsha1mq_u32(a, 7u, b)), sum(vsha1pq_u32(a, 9u, b)),
         vsha1h_u32(0xdeadbeefu), sum(vsha1su0q_u32(a, b, c)),
         sum(vsha1su1q_u32(a, b)));
  uint8x16_t x = vreinterpretq_u8_u32(a), y = vreinterpretq_u8_u32(c);
  printf("%08x %08x %08x %08x\n", sum8(vaeseq_u8(x, y)), sum8(vaesdq_u8(x, y)),
         sum8(vaesmcq_u8(x)), sum8(vaesimcq_u8(y)));
}
