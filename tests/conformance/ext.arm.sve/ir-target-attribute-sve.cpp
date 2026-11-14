// [ACLE] A function whose target attribute, written or pushed by
// '#pragma clang attribute', enables sve takes and returns svbool_t
// although the translation unit has no sve: the function carries +sve in
// its "target-features" (and at the clang 22 level vscale_range). Found
// building open-source SIMD libraries. spec/sema.md "SVE".
// `-mno-outline-atomics`: the features must not depend on the host's
// AArch64 sysroot; `-ftrivial-auto-var-init=uninitialized`: clang 22 has no
// C++26 erroneous values.
//@ expect: ir
//@ args: --target=aarch64-unknown-linux-gnu -mno-outline-atomics -ftrivial-auto-var-init=uninitialized
// svbool_t and svptrue_b8 are spelled as <arm_sve.h> defines them, so the
// test reaches no libc header under its pinned --target.
typedef __SVBool_t svbool_t;
static __inline__ __attribute__((__always_inline__, __nodebug__,
                                 __clang_arm_builtin_alias(__builtin_sve_svptrue_b8)))
svbool_t svptrue_b8(void);
__attribute__((target("sve"))) svbool_t all(void) { svbool_t p = svptrue_b8(); return p; }
#pragma clang attribute push(__attribute__((target("sve"))), apply_to = function)
svbool_t all2(svbool_t a) { return a; }
#pragma clang attribute pop
