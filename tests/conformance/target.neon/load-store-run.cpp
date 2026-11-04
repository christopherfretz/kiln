// [driver] The NEON vld1/vst1 family moves exactly the bytes its scalar
// equivalent moves: `vld1[q]` loads n contiguous elements, `vst1[q]`
// stores them, `vld1q_lane`/`vst1q_lane` touch one lane, `vld1q_dup`
// splats one element, `vgetq_lane` reads one, `vmovn` truncates each lane
// and `vcombine` concatenates (spec/landed/codegen/WP-628.md
// "NEON load/store builtins"). WP-628.
//@ expect: run
//@ requires: target-aarch64
//@ strict-diags
//@ stdout: ok 11
extern "C" int printf(const char *, ...);
#include <arm_neon.h>
#include <stdint.h>

static int fails;
static int checks;
static void check(bool c, const char *what) {
  ++checks;
  if (!c) {
    ++fails;
    printf("FAIL %s\n", what);
  }
}

int main() {
  // Odd offsets: the pointer's element alignment is all vld1/vst1 need.
  alignas(16) uint8_t bytes[40];
  for (int i = 0; i < 40; ++i) bytes[i] = uint8_t(i * 7 + 3);

  uint8x16_t q8 = vld1q_u8(bytes + 1);
  bool ok = true;
  for (int i = 0; i < 16; ++i) ok &= q8[i] == bytes[1 + i];
  check(ok, "vld1q_u8");

  uint8x8_t d8 = vld1_u8(bytes + 3);
  ok = true;
  for (int i = 0; i < 8; ++i) ok &= d8[i] == bytes[3 + i];
  check(ok, "vld1_u8");

  float f[6] = {0, 0, 0, 0, 0, 0};
  float32x4_t four = {1.5f, -2.0f, 3.25f, 8.0f};
  vst1q_f32(f + 1, four);
  check(f[0] == 0 && f[1] == 1.5f && f[2] == -2.0f && f[3] == 3.25f && f[4] == 8.0f && f[5] == 0,
        "vst1q_f32");

  int32_t s[5] = {-1, 2, -3, 4, -5};
  int32x4_t q32 = vld1q_s32(s + 1);
  check(q32[0] == 2 && q32[1] == -3 && q32[2] == 4 && q32[3] == -5, "vld1q_s32");

  uint64_t u[3] = {0, 0, 0};
  vst1_u64(u + 1, vcreate_u64(0x0123456789abcdefull));
  check(u[0] == 0 && u[1] == 0x0123456789abcdefull && u[2] == 0, "vst1_u64");

  float one = 6.5f;
  float32x4_t dup = vld1q_dup_f32(&one);
  check(dup[0] == 6.5f && dup[1] == 6.5f && dup[2] == 6.5f && dup[3] == 6.5f, "vld1q_dup_f32");

  uint16_t h = 0xbeef;
  uint16x8_t lanes = vld1q_lane_u16(&h, vdupq_n_u16(7), 5);
  ok = true;
  for (int i = 0; i < 8; ++i) ok &= lanes[i] == (i == 5 ? 0xbeef : 7);
  check(ok, "vld1q_lane_u16");

  uint8_t out[3] = {1, 1, 1};
  vst1q_lane_u8(out + 1, q8, 9);
  check(out[0] == 1 && out[1] == bytes[10] && out[2] == 1, "vst1q_lane_u8");

  uint64x2_t q64 = vreinterpretq_u64_u8(q8);
  uint64_t lo = 0, hi = 0;
  for (int i = 7; i >= 0; --i) lo = lo << 8 | bytes[1 + i];
  for (int i = 15; i >= 8; --i) hi = hi << 8 | bytes[1 + i];
  check(vgetq_lane_u64(q64, 0) == lo && vgetq_lane_u64(q64, 1) == hi, "vgetq_lane_u64");

  uint16x8_t wide = {0x1234, 0xff01, 0x00fe, 0x8080, 1, 2, 0xabcd, 0xffff};
  uint8x8_t narrow = vmovn_u16(wide);
  ok = true;
  for (int i = 0; i < 8; ++i) ok &= narrow[i] == uint8_t(wide[i]);
  check(ok, "vmovn_u16");

  uint8x16_t both = vcombine_u8(d8, narrow);
  ok = true;
  for (int i = 0; i < 16; ++i) ok &= both[i] == (i < 8 ? d8[i] : narrow[i - 8]);
  check(ok, "vcombine_u8");

  if (fails == 0) printf("ok %d\n", checks);
}
