// [driver] The AMX `__builtin_ia32_*_internal` builtins (`<amxintrin.h>`'s
// `_tile_*_internal`, under the `__tile1024i` API) take and return tiles
// as `<256 x i32>`: each tile operand is
// `llvm.x86.cast.vector.to.tile.v256i32`, the builtin is
// `llvm.x86.<name>.internal` on `x86_amx`, and a tile result is
// `llvm.x86.cast.tile.to.vector.v256i32`, as clang's
// `EmitX86BuiltinExpr`. They used to be "not supported yet" (and
// `__has_builtin` 0). Pinned to x86-64, no headers. WP-1739.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mamx-tile -mamx-int8 -mamx-bf16 -mamx-fp16 -mamx-complex
typedef int tile __attribute__((__vector_size__(1024)));
static_assert(__has_builtin(__builtin_ia32_tdpbssd_internal));
tile load(unsigned short m, unsigned short n, const void *p, unsigned long s) {
  return __builtin_ia32_tileloadd64_internal(m, n, p, s);
}
tile load_t1(unsigned short m, unsigned short n, const void *p, unsigned long s) {
  return __builtin_ia32_tileloaddt164_internal(m, n, p, s);
}
void store(unsigned short m, unsigned short n, void *p, unsigned long s, tile t) {
  __builtin_ia32_tilestored64_internal(m, n, p, s, t);
}
tile zero(unsigned short m, unsigned short n) { return __builtin_ia32_tilezero_internal(m, n); }
void dot(tile *o, unsigned short m, unsigned short n, unsigned short k, const tile *t) {
  o[0] = __builtin_ia32_tdpbssd_internal(m, n, k, t[0], t[1], t[2]);
  o[1] = __builtin_ia32_tdpbsud_internal(m, n, k, t[0], t[1], t[2]);
  o[2] = __builtin_ia32_tdpbusd_internal(m, n, k, t[0], t[1], t[2]);
  o[3] = __builtin_ia32_tdpbuud_internal(m, n, k, t[0], t[1], t[2]);
  o[4] = __builtin_ia32_tdpbf16ps_internal(m, n, k, t[0], t[1], t[2]);
  o[5] = __builtin_ia32_tdpfp16ps_internal(m, n, k, t[0], t[1], t[2]);
  o[6] = __builtin_ia32_tcmmimfp16ps_internal(m, n, k, t[0], t[1], t[2]);
  o[7] = __builtin_ia32_tcmmrlfp16ps_internal(m, n, k, t[0], t[1], t[2]);
}
