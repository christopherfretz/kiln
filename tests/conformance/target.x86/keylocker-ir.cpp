// [driver] The Key Locker builtins (`<keylockerintrin.h>`): `encodekey*`
// store the handle's 16-byte parts through the pointer unaligned and
// return the status; `aes{enc,dec}{,wide}{128,256}kl_u8` store the result
// blocks through the out pointer when bit 0 of the status is set and
// zeroes otherwise, then return the status, as clang's
// `EmitX86BuiltinExpr`. They used to be "not supported yet" (and
// `__has_builtin` 0). Pinned to x86-64, no headers. WP-1739.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -mkl -mwidekl
typedef long long v2di __attribute__((__vector_size__(16)));
static_assert(__has_builtin(__builtin_ia32_aesenc128kl_u8));
unsigned key128(unsigned h, v2di k, void *o) { return __builtin_ia32_encodekey128_u32(h, k, o); }
unsigned key256(unsigned h, v2di lo, v2di hi, void *o) {
  return __builtin_ia32_encodekey256_u32(h, lo, hi, o);
}
unsigned char one(v2di *o, v2di d, const void *h) {
  unsigned char r = __builtin_ia32_aesenc128kl_u8(o, d, h);
  r += __builtin_ia32_aesdec128kl_u8(o, d, h);
  r += __builtin_ia32_aesenc256kl_u8(o, d, h);
  return r + __builtin_ia32_aesdec256kl_u8(o, d, h);
}
unsigned char wide(v2di *o, const v2di *i, const void *h) {
  unsigned char r = __builtin_ia32_aesencwide128kl_u8(o, i, h);
  r += __builtin_ia32_aesdecwide128kl_u8(o, i, h);
  r += __builtin_ia32_aesencwide256kl_u8(o, i, h);
  return r + __builtin_ia32_aesdecwide256kl_u8(o, i, h);
}
