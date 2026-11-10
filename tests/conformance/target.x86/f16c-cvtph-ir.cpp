// [driver] `__builtin_ia32_vcvtps2ph` / `vcvtps2ph256` (`_mm_cvtps_ph`,
// `_mm256_cvtps_ph`) are `llvm.x86.vcvtps2ph.128` / `.256` with the
// rounding immediate, as clang's `EmitX86BuiltinExpr` (spec/landed/
// codegen/WP-630.md "Generic x86 lowering"). clang 22 has no
// `vcvtph2ps*` builtins; `f16c-cvtph-ir-19.cpp` pins their clang 19
// `fpext` lowering. Pinned to x86-64 so both lanes compare with clang's
// IR. WP-1242.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu -mavx -mf16c
typedef short v8hi __attribute__((__vector_size__(16)));
typedef float v4sf __attribute__((__vector_size__(16)));
typedef float v8sf __attribute__((__vector_size__(32)));
void tohalf(v8hi *h, const v4sf *b, const v8sf *c) {
  h[0] = __builtin_ia32_vcvtps2ph(*b, 0);
  h[1] = __builtin_ia32_vcvtps2ph(*b, 4);
  h[2] = __builtin_ia32_vcvtps2ph256(*c, 3);
}
