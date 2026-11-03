// [ACLE] Constants of SVE vector type are scalable splats (llvm
// `ConstantVector::getSplat`): `v + 1`, `~v`, a scalar operand converted
// to the vector (`CK_VectorSplat`, an `insertelement` + zero-mask
// `shufflevector` for a non-constant), the zero of `-v`, the `!= 0` of a
// vector `?:` condition (an `svbool_t` condition selects by its lanes)
// and scalar `?:` arms; `splat (T c)` from LLVM 20, the `shufflevector
// (insertelement ...)` expression before. An SVE comparison is unsigned
// and its `bool` is zero-extended to the result (clang does not treat the
// sizeless types as vectors there). SVE allocas are 16-byte aligned,
// `svbool_t`/`svcount_t` 2-byte. spec/codegen.md §7u, spec/types.md §5.1.
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
// `-ftrivial-auto-var-init=uninitialized`: kiln still gives SVE locals
// the C++26 erroneous-value store that clang leaves out.
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8-a+sve2p1 -ftrivial-auto-var-init=uninitialized

typedef __SVInt8_t i8;
typedef __SVInt32_t i32;
typedef __SVUint32_t u32;
typedef __SVFloat32_t f32;
typedef __SVBool_t b;

i32 add1(i32 v) { return v + 1; }
i32 addx(i32 v, int x) { return v + x; }
i32 neg(i32 v) { return -v; }
i32 inv(i32 v) { return ~v; }
f32 fadd(f32 v) { return v * 2.0f + 1.0f; }
f32 fneg(f32 v) { return -v; }
u32 compound(u32 v, unsigned x) {
  v += 3;
  v *= 2;
  v -= x;
  return v >> 1;
}
i32 sel(i32 c, i32 x, i32 y) { return c ? x : y; }
i32 sel_scalars(i32 c) { return c ? 1 : 2; }
i8 sel_scalar_arm(i8 c, i8 x) { return c ? x : 7; }
f32 sel_float(i32 c, f32 x) { return c ? x : 0.5f; }
i8 sel_bool(b c, i8 x, i8 y) { return c ? x : y; }
i32 lt(i32 x, i32 y) { return x < y; }
i32 gt3(i32 x) { return x > 3; }
i32 feq(f32 x, f32 y) { return x == y; }
b bne(b x, b y) { return x != y; }
void locals() {
  i32 v = add1(i32());
  b p = bne(b(), b());
  __SVCount_t c;
  (void)v, (void)p, (void)c;
}
