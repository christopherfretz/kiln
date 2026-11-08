// [driver] The system-register builtins lower as clang 19's
// `EmitSpecialRegisterBuiltin`: `__builtin_arm_rsr`/`rsr64`/`rsrp` call
// `llvm.read_volatile_register.i64` with the register name (or its
// `o0:op1:CRn:CRm:op2` encoding) as metadata, truncating or `inttoptr`-ing
// the result, `wsr`/`wsr64`/`wsrp` call `llvm.write_register.i64`
// (spec/codegen.md "Target builtins"). WP-846.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
#include <stdint.h>
unsigned f_rsr() { return __builtin_arm_rsr("cntvct_el0"); }
uint64_t f_rsr64() { return __builtin_arm_rsr64("cntvct_el0"); }
void *f_rsrp() { return __builtin_arm_rsrp("tpidr_el0"); }
unsigned f_rsr_enc() { return __builtin_arm_rsr("1:3:14:0:2"); }
void f_wsr(unsigned v) { __builtin_arm_wsr("tpidr_el0", v); }
void f_wsr64(uint64_t v) { __builtin_arm_wsr64("tpidr_el0", v); }
void f_wsrp(void *v) { __builtin_arm_wsrp("tpidr_el0", v); }
void f_wsr_pstate() { __builtin_arm_wsr("daifset", 2); }
