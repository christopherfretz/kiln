// [driver] The FMA3 `__builtin_ia32_vfmaddsub{ps,pd}{,256}`
// (`_mm*_fmaddsub_*`, and `_fmsubadd`, which negates the addend in the
// header) are `llvm.x86.fma.vfmaddsub.*`, as clang's `EmitX86BuiltinExpr`
// (`EmitX86FMAExpr`; spec/landed/codegen/WP-630.md "Generic x86
// lowering"). clang 22 has no `vfmadd{ps,pd,ss3,sd3}` builtins;
// `fma-ir-19.cpp` pins their clang 19 `llvm.fma` lowering. Pinned to
// x86-64 so both lanes compare with clang's IR. WP-1242.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -mfma
typedef float v4sf __attribute__((__vector_size__(16)));
typedef float v8sf __attribute__((__vector_size__(32)));
typedef double v2df __attribute__((__vector_size__(16)));
typedef double v4df __attribute__((__vector_size__(32)));
void v128(v4sf *o, v2df *d, const v4sf *a, const v2df *b) {
  o[0] = __builtin_ia32_vfmaddsubps(a[0], a[1], a[2]);
  o[1] = __builtin_ia32_vfmaddsubps(a[0], a[1], -a[2]);
  d[0] = __builtin_ia32_vfmaddsubpd(b[0], b[1], b[2]);
}
void v256(v8sf *o, v4df *d, const v8sf *a, const v4df *b) {
  o[0] = __builtin_ia32_vfmaddsubps256(a[0], a[1], a[2]);
  d[0] = __builtin_ia32_vfmaddsubpd256(b[0], b[1], -b[2]);
}
