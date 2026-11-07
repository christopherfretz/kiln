// [driver] The ACLE builtins kiln lowers emit clang 19's IR: the CRC32
// builtins `llvm.aarch64.crc32{b,h,w,x}` and their `c` forms (8- and 16-bit
// data zero-extended to `i32`), `__builtin_arm_rbit`/`rbit64`
// `llvm.bitreverse`, `clz`/`clz64` `llvm.ctlz` with zero defined (the
// 64-bit count truncated), `cls`/`cls64` `llvm.aarch64.cls`/`cls64`, the
// hints `llvm.aarch64.hint` 0-5 and the barriers `llvm.aarch64.{dmb,dsb,isb}`
// (spec/landed/sema/WP-724.md "ACLE builtins"). WP-724.
//@ requires: clang<22
//@ requires: target-aarch64
//@ expect: ir
//@ args: -march=armv8-a+crc -ftrivial-auto-var-init=uninitialized
#include <arm_acle.h>
#include <stdint.h>
uint32_t f_crc32b(uint32_t a, uint8_t b) { return __crc32b(a, b); }
uint32_t f_crc32ch(uint32_t a, uint16_t b) { return __crc32ch(a, b); }
uint32_t f_crc32w(uint32_t a, uint32_t b) { return __crc32w(a, b); }
uint32_t f_crc32cd(uint32_t a, uint64_t b) { return __crc32cd(a, b); }
uint32_t f_rbit(uint32_t a) { return __rbit(a); }
uint64_t f_rbitll(uint64_t a) { return __rbitll(a); }
unsigned f_clz(uint32_t a) { return __clz(a); }
unsigned f_clzll(uint64_t a) { return __clzll(a); }
unsigned f_cls(uint32_t a) { return __cls(a); }
unsigned f_clsll(uint64_t a) { return __clsll(a); }
void f_hints() {
  __nop();
  __yield();
  __wfe();
  __wfi();
  __sev();
  __sevl();
}
void f_barriers() {
  __dmb(11);
  __dsb(15);
  __isb(15);
}
