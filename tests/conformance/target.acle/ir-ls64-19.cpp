// [driver] The FEAT_LS64 builtins lower as clang 19's: `__builtin_arm_ld64b`
// calls `llvm.aarch64.ld64b` and stores the eight `i64` results through the
// value pointer, `st64b`/`st64bv`/`st64bv0` load the eight words and pass
// them to `llvm.aarch64.st64b*` (spec/codegen.md "Target builtins").
// WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8.7-a+ls64
#include <stdint.h>
void f_ld64b(const void *a, uint64_t *v) { __builtin_arm_ld64b(a, v); }
void f_st64b(void *a, const uint64_t *v) { __builtin_arm_st64b(a, v); }
uint64_t f_st64bv(void *a, const uint64_t *v) { return __builtin_arm_st64bv(a, v); }
uint64_t f_st64bv0(void *a, const uint64_t *v) { return __builtin_arm_st64bv0(a, v); }
