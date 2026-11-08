// [ACLE] SVE intrinsics at run time, independent of the vector length: a
// `svwhilelt` loop that loads, adds, stores and reduces, a zeroing `_z`
// operation, a maximum reduction and the element counts. Needs an SVE CPU
// (`requires: host-sve`). spec/landed/codegen/WP-810.md (WP-810).
//@ expect: run
//@ requires: target-aarch64
//@ requires: host-sve
//@ args: -march=armv8-a+sve
//@ stdout: 4950 5150 199 1 1 7
#include <arm_sve.h>
#include <stdio.h>

int a[100], b[100];

int main() {
  for (int i = 0; i < 100; ++i) a[i] = i;
  int64_t sum = 0;
  for (int32_t i = 0; i < 100; i += (int32_t)svcntw()) {
    svbool_t pg = svwhilelt_b32_s32(i, 100);
    svint32_t v = svld1_s32(pg, a + i);
    sum += svaddv_s32(pg, v);
    svst1_s32(pg, b + i, svadd_n_s32_x(pg, svadd_s32_m(pg, v, v), 2));
  }
  int64_t sum2 = 0;
  int32_t mx = 0;
  for (int32_t i = 0; i < 100; i += (int32_t)svcntw()) {
    svbool_t pg = svwhilelt_b32_s32(i, 100);
    svint32_t v = svld1_s32(pg, b + i);
    sum2 += svaddv_s32(pg, svsub_n_s32_z(pg, v, 1));
    int32_t m = svmaxv_s32(pg, v);
    if (m > mx) mx = m;
  }
  // b[i] = 2i + 2; sum2 = sum(2i + 1) = 10000, printed as sum2 - sum.
  printf("%lld %lld %d %d %d %d\n", (long long)sum, (long long)(sum2 - sum + 100),
         mx - 1, svcntb() == 4 * svcntw(), svcntd() * 2 == svcnth() / 2,
         (int)svaddv_s32(svptrue_pat_b32(SV_VL1), svdup_n_s32(7)));
}
