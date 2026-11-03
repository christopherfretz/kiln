// [ACLE] With an SVE vector condition and two scalar arms, the arms' common
// type is splatted to the SVE vector type with the same element type and
// the condition's element count (`char` is unsigned on AArch64, so it maps to
// `__SVUint8_t`); an element type with no SVE vector (`bool` under an
// integer condition, a pointer) is an error, and a
// `void` arm (including a throw-expression) is an error at that arm
// (clang 22 `CheckSizelessVectorConditionalTypes`; clang 19 crashes or
// reports these differently). spec/types.md §5.1.
//@ requires: clang>=22
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -ferror-limit=0
//@ strict-diags
//@ diag: 22:15 error "scalar type 'bool' not supported with vector condition type 'i32' (aka '__SVInt32_t')"
//@ diag: 23:15 error "scalar type 'int *' not supported with vector condition type 'i32' (aka '__SVInt32_t')"
//@ diag: 24:17 error "GNU vector conditional operand cannot be void"
//@ diag: 25:21 error "GNU vector conditional operand cannot be a throw expression"

typedef __SVInt32_t i32; typedef __SVInt8_t i8;

void f(i32 c, i8 c8, char ch, bool bb, int *p) {
  auto r1 = c8 ? ch : ch;
  auto r2 = c ? bb : bb;
  auto r3 = c ? p : p;
  auto r4 = c ? f(c, c8, ch, bb, p) : c;
  auto r5 = c ? c : throw 1;
}
