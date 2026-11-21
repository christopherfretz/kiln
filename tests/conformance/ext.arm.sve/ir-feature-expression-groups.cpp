// [ACLE] A builtin's required-feature expression is clang's
// (`TargetFeatures::hasRequiredFeatures`, WP-1841): `|` binds loosest,
// then `,`, and a parenthesized group is one operand. clang 22's
// `svadd_za16_f16_vg1x2` needs `sme,(sme-f16f16|sme-f8f16)`, so either
// half-precision feature serves; `svbdep_u8` needs
// `(sve,sve-bitperm)|(sme,ssve-bitperm)`, so a streaming caller with
// `ssve-bitperm` serves as well as a non-streaming one with
// `sve-bitperm`. (The missing-feature error is a codegen error:
// crates/kiln_driver/tests/codegen.rs
// `builtin_feature_expressions_match_clang`.)
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sme2
typedef __UINT32_TYPE__ uint32_t;
typedef __SVUint8_t svuint8_t;
typedef __clang_svfloat16x2_t svfloat16x2_t;
__attribute__((__clang_arm_builtin_alias(__builtin_sme_svadd_za16_f16_vg1x2)))
void svadd_za16_f16_vg1x2(uint32_t, svfloat16x2_t);
__attribute__((__clang_arm_builtin_alias(__builtin_sve_svbdep_u8)))
svuint8_t svbdep_u8(svuint8_t, svuint8_t);
__attribute__((target("sme-f8f16")))
void f8(uint32_t s, svfloat16x2_t v) __arm_streaming __arm_inout("za") {
  svadd_za16_f16_vg1x2(s, v);
}
__attribute__((target("sme-f16f16")))
void f16(uint32_t s, svfloat16x2_t v) __arm_streaming __arm_inout("za") {
  svadd_za16_f16_vg1x2(s, v);
}
__attribute__((target("ssve-bitperm")))
svuint8_t streaming(svuint8_t a, svuint8_t b) __arm_streaming { return svbdep_u8(a, b); }
__attribute__((target("sve2,sve-bitperm")))
svuint8_t nonstreaming(svuint8_t a, svuint8_t b) { return svbdep_u8(a, b); }
