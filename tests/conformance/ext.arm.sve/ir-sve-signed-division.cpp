// [ACLE] The SVE vector operators `/`, `%` and `>>` on a signed element
// type are `sdiv`, `srem` and `ashr` (clang `EmitDiv`/`EmitRem`/`EmitShr`
// on the element's signedness), `udiv`, `urem` and `lshr` only on the
// unsigned ones; the comparisons are clang's `icmp` either way. A
// regression test: kiln lowered every SVE vector as unsigned (WP-1791).
// spec/codegen.md.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve
__SVInt8_t d8(__SVInt8_t a, __SVInt8_t b) { return a / b; }
__SVInt8_t r8(__SVInt8_t a, __SVInt8_t b) { return a % b; }
__SVInt8_t s8(__SVInt8_t a, __SVInt8_t b) { return a >> b; }
__SVInt8_t c8(__SVInt8_t a, __SVInt8_t b) { return a < b; }
__SVInt16_t d16(__SVInt16_t a, __SVInt16_t b) { return a / b; }
__SVInt32_t d32(__SVInt32_t a) { return a / 3; }
__SVInt64_t r64(__SVInt64_t a, __SVInt64_t b) { return a % b; }
__SVInt64_t s64(__SVInt64_t a) { return a >> 2; }
__SVUint8_t ud8(__SVUint8_t a, __SVUint8_t b) { return a / b; }
__SVUint32_t ur32(__SVUint32_t a, __SVUint32_t b) { return a % b; }
__SVUint64_t us64(__SVUint64_t a, __SVUint64_t b) { return a >> b; }
__SVInt16_t uc16(__SVUint16_t a, __SVUint16_t b) { return a < b; }
__SVFloat32_t fd32(__SVFloat32_t a, __SVFloat32_t b) { return a / b; }
