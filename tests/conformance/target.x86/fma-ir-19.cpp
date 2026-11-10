// [driver] The FMA3 builtins `__builtin_ia32_vfmadd{ps,pd}{,256}` and the
// scalar `vfmadd{ss,sd}3` (`_mm*_fmadd_*`, and `_fmsub`/`_fnmadd`/
// `_fnmsub`, which negate operands in the header), and the FMA4 scalar
// `vfmadd{ss,sd}`, are generic IR, as clang's `EmitX86BuiltinExpr`
// (`EmitX86FMAExpr`, `EmitScalarFMAExpr`): `llvm.fma.*` on the operands
// (a negated operand an `fneg` before it); the FMA3 scalar form is
// `llvm.fma.f32`/`f64` on lane 0 inserted back into the first operand,
// the FMA4 one into a zero vector; `vfmaddsub*` stay
// `llvm.x86.fma.vfmaddsub.*` (spec/landed/codegen/WP-630.md "Generic x86
// lowering"). clang 22 has no `vfmadd{ps,pd,ss3,sd3,ss,sd}` builtins (its
// header uses `__builtin_elementwise_fma`; kiln keeps clang 19's table and
// headers), so this is the clang 19 module only; `fma-ir.cpp` pins the
// clang 22 `vfmaddsub*`. Pinned to x86-64 so both lanes compare with
// clang 19's IR. WP-1242.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -mfma -mfma4
typedef float v4sf __attribute__((__vector_size__(16)));
typedef float v8sf __attribute__((__vector_size__(32)));
typedef double v2df __attribute__((__vector_size__(16)));
typedef double v4df __attribute__((__vector_size__(32)));
void v128(v4sf *o, v2df *d, const v4sf *a, const v2df *b) {
  o[0] = __builtin_ia32_vfmaddps(a[0], a[1], a[2]);
  o[1] = __builtin_ia32_vfmaddps(a[0], a[1], -a[2]);
  o[2] = __builtin_ia32_vfmaddps(-a[0], a[1], a[2]);
  o[3] = __builtin_ia32_vfmaddsubps(a[0], a[1], a[2]);
  o[4] = __builtin_ia32_vfmaddsubps(a[0], a[1], -a[2]);
  d[0] = __builtin_ia32_vfmaddpd(b[0], b[1], b[2]);
  d[1] = __builtin_ia32_vfmaddpd(-b[0], b[1], -b[2]);
  d[2] = __builtin_ia32_vfmaddsubpd(b[0], b[1], b[2]);
}
void v256(v8sf *o, v4df *d, const v8sf *a, const v4df *b) {
  o[0] = __builtin_ia32_vfmaddps256(a[0], a[1], a[2]);
  o[1] = __builtin_ia32_vfmaddps256(-a[0], a[1], -a[2]);
  o[2] = __builtin_ia32_vfmaddsubps256(a[0], a[1], a[2]);
  d[0] = __builtin_ia32_vfmaddpd256(b[0], b[1], b[2]);
  d[1] = __builtin_ia32_vfmaddpd256(b[0], b[1], -b[2]);
  d[2] = __builtin_ia32_vfmaddsubpd256(b[0], b[1], -b[2]);
}
void scalar(v4sf *o, v2df *d, const v4sf *a, const v2df *b) {
  o[0] = __builtin_ia32_vfmaddss3(a[0], a[1], a[2]);
  o[1] = __builtin_ia32_vfmaddss3(a[0], -a[1], -a[2]);
  d[0] = __builtin_ia32_vfmaddsd3(b[0], b[1], -b[2]);
  d[1] = __builtin_ia32_vfmaddsd3(b[0], -b[1], b[2]);
}
void fma4(v4sf *o, v2df *d, const v4sf *a, const v2df *b) {
  o[0] = __builtin_ia32_vfmaddss(a[0], a[1], a[2]);
  d[0] = __builtin_ia32_vfmaddsd(b[0], -b[1], b[2]);
}
