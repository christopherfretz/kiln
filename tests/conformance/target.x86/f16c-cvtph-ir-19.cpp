// [driver] `__builtin_ia32_vcvtph2ps` / `vcvtph2ps256` (`_mm_cvtph_ps`,
// `_mm256_cvtph_ps`) are generic IR, as clang's `EmitX86BuiltinExpr`
// (`EmitX86CvtF16ToFloatExpr`): the `<8 x i16>` operand bitcast to
// `<8 x half>` (its low four lanes shuffled out for the 128-bit form) and
// `fpext`ed to float; `vcvtps2ph` / `vcvtps2ph256` stay
// `llvm.x86.vcvtps2ph.*` (spec/landed/codegen/WP-630.md "Generic x86
// lowering"). clang 22 has no `vcvtph2ps*` builtins (its header converts
// with `__builtin_convertvector`; kiln keeps clang 19's table and
// headers), so this is the clang 19 module only; `f16c-cvtph-ir.cpp` pins
// the clang 22 `vcvtps2ph*`. Pinned to x86-64 so both lanes compare with
// clang 19's IR. WP-1242.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu -mavx -mf16c
typedef short v8hi __attribute__((__vector_size__(16)));
typedef float v4sf __attribute__((__vector_size__(16)));
typedef float v8sf __attribute__((__vector_size__(32)));
void toflt(v4sf *f, v8sf *g, const v8hi *a) {
  f[0] = __builtin_ia32_vcvtph2ps(*a);
  g[0] = __builtin_ia32_vcvtph2ps256(*a);
}
void tohalf(v8hi *h, const v4sf *b, const v8sf *c) {
  h[0] = __builtin_ia32_vcvtps2ph(*b, 0);
  h[1] = __builtin_ia32_vcvtps2ph(*b, 4);
  h[2] = __builtin_ia32_vcvtps2ph256(*c, 3);
}
