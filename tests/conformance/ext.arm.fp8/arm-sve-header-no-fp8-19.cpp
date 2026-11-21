// [ACLE] At clang 19's compatibility level the clang 19 <arm_sve.h> serves
// (WP-1842): it declares no FP8 type or intrinsic, and `__mfp8` and
// `__SVMfloat8_t` are not types (WP-1791). Host-restricted (no pinned
// `--target`): the header reaches <stdint.h>.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: compile-fail
//@ args: -march=armv9-a+sve2
//@ strict-diags
//@ diag: 16:8 error "unknown type name 'svmfloat8_t'"
//@ diag: 17:8 error "unknown type name 'fpm_t'"
#include <arm_sve.h>
svint8_t ok(svint8_t a) { return a; }
// Ordinary identifiers at this level.
int __mfp8 = 1, __SVMfloat8_t = 2, __clang_svmfloat8x2_t = 3;
void f(svmfloat8_t a);
void k(fpm_t a);
