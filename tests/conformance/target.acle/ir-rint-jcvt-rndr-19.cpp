// [driver] The FEAT_FRINTTS, JSCVT and RNG builtins lower as clang 19's:
// `__builtin_arm_rint{32,64}{x,z}[f]` call `llvm.aarch64.frint{32,64}{x,z}`,
// `jcvt` calls `llvm.aarch64.fjcvtzs`, and `rndr`/`rndrrs` call
// `llvm.aarch64.rndr`/`rndrrs`, store the value through the pointer and
// return the zero-extended flag (spec/codegen.md "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8.5-a+rng
#include <stdint.h>
double f_rint32x(double x) { return __builtin_arm_rint32x(x); }
float f_rint32xf(float x) { return __builtin_arm_rint32xf(x); }
double f_rint64z(double x) { return __builtin_arm_rint64z(x); }
float f_rint64zf(float x) { return __builtin_arm_rint64zf(x); }
int f_jcvt(double x) { return __builtin_arm_jcvt(x); }
int f_rndr(uint64_t *p) { return __builtin_arm_rndr(p); }
int f_rndrrs(uint64_t *p) { return __builtin_arm_rndrrs(p); }
