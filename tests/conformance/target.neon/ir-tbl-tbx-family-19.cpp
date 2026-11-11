// [driver] The rest of the table-lookup family emits clang 19's IR: `vqtbl2`..`vqtbl4`
// (`q` and `d` result) to `tbl2`..`tbl4`, `vqtbx1`..`vqtbx4` to `tbx1`..`tbx4`, `vtbl3`/`vtbl4`
// to `tbl2` over the 64-bit tables paired by shuffles (the odd one zero-padded), `vtbx2`/`vtbx4`
// to `tbx1`/`tbx2` over the pairs, and `vtbx1`/`vtbx3` to `tbl1`/`tbl2` with the lanes whose
// index is out of range taken from the accumulator, as clang 19's `EmitAArch64TblBuiltinExpr`
// (spec/landed/codegen/WP-1288.md "NEON table lookups"). The clang 19 IR (clang 22's
// arm_neon.h adds a temporary per call). WP-1288.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -ftrivial-auto-var-init=uninitialized
#include <arm_neon.h>
auto q2(uint8x16x2_t t, uint8x16_t i) { return vqtbl2q_u8(t, i); }
auto d2(uint8x16x2_t t, uint8x8_t i) { return vqtbl2_u8(t, i); }
auto q3(int8x16x3_t t, uint8x16_t i) { return vqtbl3q_s8(t, i); }
auto d3(uint8x16x3_t t, uint8x8_t i) { return vqtbl3_u8(t, i); }
auto q4(uint8x16x4_t t, uint8x16_t i) { return vqtbl4q_u8(t, i); }
auto d4(poly8x16x4_t t, uint8x8_t i) { return vqtbl4_p8(t, i); }
auto x1(uint8x8_t a, uint8x16_t t, uint8x8_t i) { return vqtbx1_u8(a, t, i); }
auto x1q(uint8x16_t a, uint8x16_t t, uint8x16_t i) { return vqtbx1q_u8(a, t, i); }
auto x2(int8x8_t a, int8x16x2_t t, uint8x8_t i) { return vqtbx2_s8(a, t, i); }
auto x2q(uint8x16_t a, uint8x16x2_t t, uint8x16_t i) { return vqtbx2q_u8(a, t, i); }
auto x3(uint8x8_t a, uint8x16x3_t t, uint8x8_t i) { return vqtbx3_u8(a, t, i); }
auto x3q(uint8x16_t a, uint8x16x3_t t, uint8x16_t i) { return vqtbx3q_u8(a, t, i); }
auto x4(uint8x8_t a, uint8x16x4_t t, uint8x8_t i) { return vqtbx4_u8(a, t, i); }
auto x4q(uint8x16_t a, uint8x16x4_t t, uint8x16_t i) { return vqtbx4q_u8(a, t, i); }
auto t3(uint8x8x3_t t, uint8x8_t i) { return vtbl3_u8(t, i); }
auto t4(int8x8x4_t t, int8x8_t i) { return vtbl4_s8(t, i); }
auto b1(uint8x8_t a, uint8x8_t t, uint8x8_t i) { return vtbx1_u8(a, t, i); }
auto b2(uint8x8_t a, uint8x8x2_t t, uint8x8_t i) { return vtbx2_u8(a, t, i); }
auto b3(int8x8_t a, int8x8x3_t t, int8x8_t i) { return vtbx3_s8(a, t, i); }
auto b4(poly8x8_t a, poly8x8x4_t t, uint8x8_t i) { return vtbx4_p8(a, t, i); }
