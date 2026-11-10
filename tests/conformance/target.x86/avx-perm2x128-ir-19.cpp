// [driver] `__builtin_ia32_permti256` (`_mm256_permute2x128_si256`) and
// `vperm2f128_{ps,pd,si}256` (`_mm256_permute2f128_*`) are a
// `shufflevector` of the two sources, as clang's `EmitX86BuiltinExpr`:
// each 128-bit half of the result is the half of `a:b` the immediate's
// nibble selects, or zero (a `zeroinitializer` operand) when its bit 3 is
// set (spec/landed/codegen/WP-630.md "Generic x86 lowering"). The builtins
// are called directly (kiln's resource headers are clang 19's). Pinned to
// x86-64 so both lanes compare with clang 19's IR; sibling of the clang 22 test. WP-1242.
//@ expect: ir
//@ requires: clang<22
//@ args: --target=x86_64-unknown-linux-gnu -mavx2
typedef long long v4di __attribute__((__vector_size__(32)));
typedef int v8si __attribute__((__vector_size__(32)));
typedef float v8sf __attribute__((__vector_size__(32)));
typedef double v4df __attribute__((__vector_size__(32)));
void permi(v4di *o, const v4di *a) {
  o[0] = __builtin_ia32_permti256(a[0], a[1], 0x20);
  o[1] = __builtin_ia32_permti256(a[0], a[1], 0x31);
  o[2] = __builtin_ia32_permti256(a[0], a[1], 0x12);
  o[3] = __builtin_ia32_permti256(a[0], a[1], 0x83);
  o[4] = __builtin_ia32_permti256(a[0], a[1], 0x08);
  o[5] = __builtin_ia32_permti256(a[0], a[1], 0x88);
  o[6] = __builtin_ia32_permti256(a[0], a[1], 0x00);
  o[7] = __builtin_ia32_permti256(a[0], a[1], 0x33);
}
void permf(v8si *o, v8sf *f, v4df *d, const v8si *a, const v8sf *b, const v4df *c) {
  o[0] = __builtin_ia32_vperm2f128_si256(a[0], a[1], 0x21);
  f[0] = __builtin_ia32_vperm2f128_ps256(b[0], b[1], 0x03);
  f[1] = __builtin_ia32_vperm2f128_ps256(b[0], b[1], 0x8a);
  d[0] = __builtin_ia32_vperm2f128_pd256(c[0], c[1], 0x30);
  d[1] = __builtin_ia32_vperm2f128_pd256(c[0], c[1], 0xff);
}
