// [driver] The AVX-512 mask builtins clang lowers to generic IR in
// `EmitX86BuiltinExpr`: a `select` builtin a `select` on the `bitcast`
// mask (`EmitX86Select`), an integer compare an `icmp` whose `<N x i1>` is
// the mask (`EmitX86MaskedCompare`), a masked load/store
// `llvm.masked.load` / `llvm.masked.store` (clang 22: the alignment a
// call-site `align` attribute), a float reduction
// `llvm.vector.reduce.*`, a `kand` an `and` of `<16 x i1>`, `pternlog`
// `llvm.x86.avx512.pternlog.d.512` and a `select`
// (spec/landed/codegen/WP-630.md "Generic x86 lowering"). Each function
// is "min-legal-vector-width"="512", clang's required width of the
// builtins it calls: under `-march=skylake-avx512` (which prefers 256-bit
// vectors) a smaller width made the backend split the 512-bit intrinsics
// and abort. clang 22 has no `evex512` feature, so at `-fclang-compat=22`
// the `,evex512` term of the clang 19 table's requirement is dropped. The
// builtins are called directly (kiln's resource headers are clang 19's).
// Pinned to x86-64 so both lanes compare with clang 19's IR; the
// sibling of the clang 22 test (the clang 19 module: the masked
// load/store alignment an `i32` operand). WP-1242.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu -march=skylake-avx512
typedef int v8si __attribute__((__vector_size__(32)));
typedef int v16si __attribute__((__vector_size__(64)));
typedef char v64qi __attribute__((__vector_size__(64)));
typedef float v16sf __attribute__((__vector_size__(64)));
void sel(v16si *o, v64qi *b, const v16si *a, const v64qi *c, unsigned short k, unsigned long long kb) {
  o[0] = __builtin_ia32_selectd_512(k, a[0], a[1]);
  b[0] = __builtin_ia32_selectb_512(kb, c[0], c[1]);
  o[1] = __builtin_ia32_pternlogd512_mask(a[0], a[1], a[2], 0x96, k);
  o[2] = __builtin_ia32_pternlogd512_mask(a[0], a[1], a[2], 0xca, (unsigned short)-1);
}
unsigned cmp(const v16si *a, const v8si *b, unsigned short k) {
  unsigned short m = __builtin_ia32_cmpd512_mask(a[0], a[1], 1, (unsigned short)-1);
  unsigned short n = __builtin_ia32_ucmpd512_mask(a[0], a[1], 4, k);
  unsigned char p = __builtin_ia32_cmpd256_mask(b[0], b[1], 0, 0xff);
  return __builtin_ia32_kandhi(m, n) + p;
}
void mem(v16si *o, int *q, const int *p, v16si *r, const v16si *a, unsigned short k) {
  o[0] = __builtin_ia32_loaddqusi512_mask(p, a[0], k);
  o[1] = __builtin_ia32_movdqa32load512_mask(a, a[1], k);
  __builtin_ia32_storedqusi512_mask(q, a[1], k);
  __builtin_ia32_movdqa32store512_mask(r, a[1], k);
}
float red(const v16sf *f) {
  return __builtin_ia32_reduce_fadd_ps512(1.0f, f[0]) + __builtin_ia32_reduce_fmin_ps512(f[1]);
}
