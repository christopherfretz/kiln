// A GNU vector shifted by a scalar count of another integer width: the
// splatted count is converted element-wise to the vector's element type
// (clang EmitShl/EmitShr, CreateIntCast unsigned), `<2 x i32>` to
// `<2 x i64>` for `v2u64 << int` and `<16 x i64>` to `<16 x i8>` for
// `v16u8 << long`. BoringSSL's Keccak rotate failed IR lowering. WP-811.
//@ expect: run
//@ strict-diags
//@ stdout: 18 30 4 6 6
#include <cstdio>
typedef unsigned long long u64;
typedef u64 v2u64 __attribute__((vector_size(16)));
typedef unsigned char v16u8 __attribute__((vector_size(16)));
static v2u64 rotl(v2u64 x, int s) { return (x << s) | (x >> ((-s) & 63)); }
int main(int argc, char **) {
  v2u64 x = {0x8000000000000001ull, 3};
  v2u64 y = rotl(x, argc + 3);
  v16u8 b = {1, 2, 3};
  long sh = argc;
  v16u8 c = b << sh;
  x <<= argc;
  std::printf("%llx %llx %d %d %llx\n", y[0], y[1], c[1], c[2], x[1]);
}
