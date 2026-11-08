// [driver] The guarded-control-stack, `chkfeat`, `prefetch` and `trap`
// builtins lower as clang 19's: `__builtin_arm_gcspopm`/`gcsss` call
// `llvm.aarch64.gcspopm`/`gcsss`, `chkfeat` calls `llvm.aarch64.chkfeat`,
// `prefetch` calls `llvm.aarch64.prefetch` with its four immediates, and
// `trap` calls the `noreturn` `llvm.aarch64.break` (spec/codegen.md
// "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv9.4-a+gcs
#include <stdint.h>
uint64_t f_gcspopm(uint64_t v) { return __builtin_arm_gcspopm(v); }
const void *f_gcsss(const void *p) { return __builtin_arm_gcsss(p); }
uint64_t f_chkfeat(uint64_t v) { return __builtin_arm_chkfeat(v); }
void f_prefetch(const void *p) { __builtin_arm_prefetch(p, 1, 2, 0, 1); __builtin_arm_prefetch(p, 0, 0, 1, 0); }
void f_trap() { __builtin_arm_trap(42); }
