// [AAPCS64 §5.4.2] SVE tuple types by value (clang 20+): a tuple is a
// struct of its vectors in IR, passed as one scalable argument per vector
// (gathered into the struct with `insertvalue` in the callee, split with
// `extractvalue` at the call) while its Z or P registers are free, else
// by reference, and returned as the struct. clang 19 instead used one
// wide scalable vector (ir-sve-builtins-19.cpp). IR only: the test host
// has no SVE (WP-1739).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve -msve-vector-bits=128
typedef __SVInt32_t svint32_t;
void tup(__clang_svint8x2_t t, __clang_svboolx2_t p);
__clang_svint8x2_t rtup(__clang_svint8x2_t t) { return t; }
void tc(__clang_svint8x2_t t, __clang_svboolx2_t p) { tup(t, p); }
__clang_svint32x3_t t3(__clang_svint32x3_t a, svint32_t b) { return a; }
__clang_svint32x4_t t4(__clang_svint32x4_t a, __clang_svint32x4_t b, __clang_svint32x2_t c) {
  return b;
}
__clang_svboolx4_t p4(__clang_svboolx4_t a, __clang_svboolx2_t b) { return a; }
__clang_svfloat64x2_t f2(double x, __clang_svfloat64x2_t a) { return a; }
void calls(__clang_svint32x4_t a, __clang_svint32x2_t c, __clang_svboolx4_t p,
           __clang_svboolx2_t q) {
  t4(a, a, c);
  p4(p, q);
}
