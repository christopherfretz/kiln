// [ACLE] Lowering of the FP8 types (WP-1791, clang 22's; clang 19 has
// none): `__mfp8` is `<1 x i8>` in registers and in memory, passed and
// returned direct without `noundef` and without using an FP register (a
// struct of four is an i64, not a homogeneous aggregate); a NEON vector of
// it is `<8 x i8>` / `<16 x i8>`; `__SVMfloat8_t` is `<vscale x 16 x i8>`
// and its `+ - * /` and `<` are the signed SVE integer operations
// (`sdiv`, `icmp ult` widened to an `__SVInt8_t`); the mangled names are
// `u6__mfp8` (never a substitution candidate), `u13__SVMfloat8_t`,
// `13svmfloat8x2_t` and `13__Mfloat8x8_t`; `typeid(__mfp8)` emits
// `_ZTIu6__mfp8` where used. spec/types.md §5.1, spec/abi.md.
// `-ftrivial-auto-var-init=uninitialized`: clang has no C++26 erroneous
// values.
//@ requires: clang>=22
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -march=armv9-a+sve2+fp8 -ftrivial-auto-var-init=uninitialized
namespace std {
class type_info;
}
typedef __attribute__((neon_vector_type(8))) __mfp8 m8x8;
typedef __attribute__((neon_vector_type(16))) __mfp8 m8x16;
__mfp8 g;
__mfp8 garr[4];
struct S { __mfp8 a; char b; };
struct T { __mfp8 a[4]; };
struct H { __mfp8 a, b, c, d; };
S gs;
void f(__mfp8, __SVMfloat8_t, __clang_svmfloat8x2_t, __clang_svmfloat8x3_t,
       __clang_svmfloat8x4_t, m8x8, m8x16) {}
void twice(__mfp8, __mfp8) {}
__mfp8 id(__mfp8 x) {
  __mfp8 y = x;
  y = x;
  return y;
}
__mfp8 *next(__mfp8 *p) { return p + 1; }
template <class U> void tf(U) {}
template void tf<__mfp8>(__mfp8);
S fs(S s) { return s; }
T ft(T t) { return t; }
H fh(H h) { return h; }
void regs(__mfp8 m, __clang_svint8x4_t a, __clang_svint8x4_t b) {}
void hfa(H, __clang_svint8x4_t a, __clang_svint8x4_t b) {}
void vf(int n, ...);
void variadic(__mfp8 m) { vf(1, m); }
const std::type_info &ti() { return typeid(__mfp8); }
__mfp8 from_byte(unsigned char u) { return __builtin_bit_cast(__mfp8, u); }
unsigned char to_byte(__mfp8 m) { return __builtin_bit_cast(unsigned char, m); }
m8x8 neon(m8x8 x) { return -x; }
__mfp8 lane(m8x16 x) { return x[3]; }
__SVMfloat8_t idv(__SVMfloat8_t x) {
  __SVMfloat8_t y = x;
  return y;
}
__SVMfloat8_t add(__SVMfloat8_t v, __SVMfloat8_t w) { return v + w; }
__SVMfloat8_t sub(__SVMfloat8_t v, __SVMfloat8_t w) { return v - w; }
__SVMfloat8_t mul(__SVMfloat8_t v, __SVMfloat8_t w) { return v * w; }
__SVMfloat8_t div(__SVMfloat8_t v, __SVMfloat8_t w) { return v / w; }
__SVMfloat8_t neg(__SVMfloat8_t v) { return -v; }
__SVMfloat8_t pos(__SVMfloat8_t v) { return +v; }
__SVInt8_t lt(__SVMfloat8_t v, __SVMfloat8_t w) { return v < w; }
__mfp8 elt(__SVMfloat8_t v) { return v[1]; }
