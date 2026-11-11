// [driver] The table lookups `vqtbl2`..`vqtbl4`, `vqtbx1`..`vqtbx4`, `vtbl3`/`vtbl4` and
// `vtbx1`..`vtbx4` agree with scalar code at -O0: each lane is the table byte its index names,
// or 0 (`tbl`) or the accumulator lane (`tbx`) when the index is past the table
// (spec/landed/codegen/WP-1288.md "NEON table lookups"). WP-1288.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0
//@ strict-diags
//@ stdout: ok 15
extern "C" int printf(const char *, ...);
#include <arm_neon.h>
#include <stdint.h>

static int fails;
static int checks;
static uint8_t tab[64];
static uint8_t idx[16] = {0, 5, 17, 31, 32, 47, 48, 63, 64, 200, 9, 24, 40, 55, 255, 1};
static uint8_t acc[16] = {90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105};

// The expected lane: `n` table bytes, `tbx` keeps the accumulator.
static uint8_t want(int lane, int n, bool tbx) {
  uint8_t i = idx[lane];
  if (i < n) return tab[i];
  return tbx ? acc[lane] : 0;
}

static void check(const uint8_t *got, int lanes, int n, bool tbx) {
  ++checks;
  for (int l = 0; l < lanes; ++l)
    if (got[l] != want(l, n, tbx)) {
      printf("check %d lane %d: %d, want %d\n", checks, l, got[l], want(l, n, tbx));
      ++fails;
    }
}

int main() {
  for (int k = 0; k < 64; ++k) tab[k] = (uint8_t)(k * 3 + 1);
  uint8_t out[16];
  uint8x16_t iq = vld1q_u8(idx), aq = vld1q_u8(acc);
  uint8x8_t id = vld1_u8(idx), ad = vld1_u8(acc);
  uint8x16x2_t q2 = {{vld1q_u8(tab), vld1q_u8(tab + 16)}};
  uint8x16x3_t q3 = {{q2.val[0], q2.val[1], vld1q_u8(tab + 32)}};
  uint8x16x4_t q4 = {{q3.val[0], q3.val[1], q3.val[2], vld1q_u8(tab + 48)}};
  uint8x8x2_t d2 = {{vld1_u8(tab), vld1_u8(tab + 8)}};
  uint8x8x3_t d3 = {{d2.val[0], d2.val[1], vld1_u8(tab + 16)}};
  uint8x8x4_t d4 = {{d3.val[0], d3.val[1], d3.val[2], vld1_u8(tab + 24)}};

  vst1q_u8(out, vqtbl2q_u8(q2, iq)); check(out, 16, 32, false);
  vst1_u8(out, vqtbl3_u8(q3, id)); check(out, 8, 48, false);
  vst1q_u8(out, vqtbl4q_u8(q4, iq)); check(out, 16, 64, false);
  vst1_u8(out, vqtbx1_u8(ad, q2.val[0], id)); check(out, 8, 16, true);
  vst1q_u8(out, vqtbx2q_u8(aq, q2, iq)); check(out, 16, 32, true);
  vst1q_u8(out, vqtbx3q_u8(aq, q3, iq)); check(out, 16, 48, true);
  vst1_u8(out, vqtbx4_u8(ad, q4, id)); check(out, 8, 64, true);
  vst1q_u8(out, vqtbx1q_u8(aq, q4.val[0], iq)); check(out, 16, 16, true);
  vst1_u8(out, vqtbl2_u8(q2, id)); check(out, 8, 32, false);
  vst1_u8(out, vtbl3_u8(d3, id)); check(out, 8, 24, false);
  vst1_u8(out, vtbl4_u8(d4, id)); check(out, 8, 32, false);
  vst1_u8(out, vtbx1_u8(ad, d2.val[0], id)); check(out, 8, 8, true);
  vst1_u8(out, vtbx2_u8(ad, d2, id)); check(out, 8, 16, true);
  vst1_u8(out, vtbx3_u8(ad, d3, id)); check(out, 8, 24, true);
  vst1_u8(out, vtbx4_u8(ad, d4, id)); check(out, 8, 32, true);
  if (fails == 0) printf("ok %d\n", checks);
  return fails;
}
