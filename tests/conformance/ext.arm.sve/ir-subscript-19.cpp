// [expr.sub] [ACLE] `v[i]` on an SVE vector is a vector element
// (`LValue::MakeVectorElt`): a read is `extractelement` of the loaded
// vector at the unpromoted index, a write `insertelement` into a fresh
// load stored back; `++`/`+=` do both. spec/codegen.md §7aa, spec/sema.md
// "Vector subscript".
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
// The clang 19 output (clang 22 differs; sibling of the clang 22 test).
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8-a+sve

typedef __SVInt32_t i32;
typedef __SVFloat32_t f32;
typedef __SVUint8_t u8;

int get(i32 v, int i) { return v[i]; }
int get2(i32 v) { return v[2]; }
float getf(f32 v, long i) { return v[i]; }
unsigned getu(u8 v, unsigned char i) { return v[i]; }
void set(i32 &v, int i) { v[i] = 3; }
void inc(i32 v, int i) {
  v[i]++;
  v[i] += 2;
}
int temp(int i) { return i32()[i]; }
