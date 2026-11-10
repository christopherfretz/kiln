// [driver] `__builtin_ia32_extract128i256` / `vextractf128_{ps,pd,si}256`
// (`_mm256_extracti128_si256`, `_mm256_extractf128_*`) and
// `insert128i256` / `vinsertf128_*` (`_mm256_inserti128_si256`,
// `_mm256_insertf128_*`) are generic IR, as clang's `EmitX86BuiltinExpr`:
// the extract a `shufflevector` of the half the immediate's low bit names,
// the insert a widening `shufflevector` and a second one taking that half
// from it (spec/landed/codegen/WP-630.md "Generic x86 lowering"). The
// builtins are called directly (kiln's resource headers are clang 19's;
// an immediate past 1 is masked to its low bit, as clang)
// and the function's "min-legal-vector-width" is clang's required width of
// each builtin (128 for a lone `extract128i256` at clang 22). Pinned to
// x86-64 so both lanes compare with clang's IR. WP-1242.
//@ expect: ir
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu -mavx2 -Wno-argument-outside-range
typedef long long v2di __attribute__((__vector_size__(16)));
typedef long long v4di __attribute__((__vector_size__(32)));
typedef int v4si __attribute__((__vector_size__(16)));
typedef int v8si __attribute__((__vector_size__(32)));
typedef float v4sf __attribute__((__vector_size__(16)));
typedef float v8sf __attribute__((__vector_size__(32)));
typedef double v2df __attribute__((__vector_size__(16)));
typedef double v4df __attribute__((__vector_size__(32)));
void exti(v2di *o, const v4di *a) {
  o[0] = __builtin_ia32_extract128i256(*a, 0);
  o[1] = __builtin_ia32_extract128i256(*a, 1);
  o[2] = __builtin_ia32_extract128i256(*a, 3);
}
void extf(v4si *o, v4sf *f, v2df *d, const v8si *a, const v8sf *b, const v4df *c) {
  o[0] = __builtin_ia32_vextractf128_si256(*a, 1);
  f[0] = __builtin_ia32_vextractf128_ps256(*b, 1);
  f[1] = __builtin_ia32_vextractf128_ps256(*b, 0);
  d[0] = __builtin_ia32_vextractf128_pd256(*c, 0);
  d[1] = __builtin_ia32_vextractf128_pd256(*c, 1);
}
void ins(v4di *o, v8si *s, v8sf *f, v4df *d, const v2di *a, const v4si *t, const v4sf *b, const v2df *c) {
  o[0] = __builtin_ia32_insert128i256(o[0], *a, 0);
  o[1] = __builtin_ia32_insert128i256(o[1], *a, 1);
  s[0] = __builtin_ia32_vinsertf128_si256(s[0], *t, 1);
  f[0] = __builtin_ia32_vinsertf128_ps256(f[0], *b, 0);
  f[1] = __builtin_ia32_vinsertf128_ps256(f[1], *b, 3);
  d[0] = __builtin_ia32_vinsertf128_pd256(d[0], *c, 1);
}
