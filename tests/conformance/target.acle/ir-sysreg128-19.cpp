// [driver] The 128-bit system-register builtins `__builtin_arm_rsr128` /
// `wsr128` (FEAT_D128) call `llvm.read_volatile_register.i128` /
// `llvm.write_register.i128` (spec/codegen.md "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv9.4-a+d128
unsigned __int128 f_rsr128() { return __builtin_arm_rsr128("1:2:3:4:5"); }
unsigned __int128 f_rsr128n() { return __builtin_arm_rsr128("ttbr0_el1"); }
void f_wsr128(unsigned __int128 v) { __builtin_arm_wsr128("ttbr0_el1", v); }
