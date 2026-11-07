// [driver] The ACLE data-processing and hint builtins kiln lowers agree
// with their definitions at -O0: the CRC32 and CRC32C checksums at every
// width (the 8- and 16-bit data zero-extended), `__rbit`/`__rbitll`,
// `__clz`/`__clzll` (32 and 64 at zero), `__cls`/`__clsll`, and the hint
// and barrier instructions (spec/landed/sema/WP-724.md "ACLE builtins").
// WP-724.
//@ expect: run
//@ requires: target-aarch64
//@ args: -O0 -march=armv8-a+crc
//@ strict-diags
//@ stdout: ok 17
extern "C" int printf(const char *, ...);
#include <arm_acle.h>
#include <stdint.h>

static int fails;
static int checks;
static void check(bool c, const char *what) {
  ++checks;
  if (!c) {
    ++fails;
    printf("FAIL %s\n", what);
  }
}

// Bitwise reference CRC (reflected, polynomial `poly`), `n` data bytes.
static uint32_t ref(uint32_t crc, uint64_t data, int n, uint32_t poly) {
  for (int i = 0; i < n; ++i) {
    crc ^= (uint32_t)(data >> (8 * i)) & 0xff;
    for (int k = 0; k < 8; ++k)
      crc = (crc >> 1) ^ ((crc & 1) ? poly : 0);
  }
  return crc;
}

int main() {
  const uint32_t C = 0x82f63b78, Z = 0xedb88320;
  uint32_t a = 0x12345678;
  uint64_t d = 0x0123456789abcdefull;
  check(__crc32b(a, 0xa5) == ref(a, 0xa5, 1, Z), "crc32b");
  check(__crc32h(a, 0xbeef) == ref(a, 0xbeef, 2, Z), "crc32h");
  check(__crc32w(a, 0xdeadbeef) == ref(a, 0xdeadbeef, 4, Z), "crc32w");
  check(__crc32d(a, d) == ref(a, d, 8, Z), "crc32d");
  check(__crc32cb(a, 0xa5) == ref(a, 0xa5, 1, C), "crc32cb");
  check(__crc32ch(a, 0xbeef) == ref(a, 0xbeef, 2, C), "crc32ch");
  check(__crc32cw(a, 0xdeadbeef) == ref(a, 0xdeadbeef, 4, C), "crc32cw");
  check(__crc32cd(a, d) == ref(a, d, 8, C), "crc32cd");
  check(__rbit(1u) == 0x80000000u, "rbit");
  check(__rbitll(0x3ull) == 0xc000000000000000ull, "rbitll");
  volatile uint32_t zero = 0;
  check(__clz(zero) == 32 && __clz(1u) == 31, "clz");
  check(__clzll((uint64_t)zero) == 64 && __clzll(1ull << 40) == 23, "clzll");
  check(__cls(0) == 31 && __cls(-1) == 31 && __cls(1) == 30, "cls");
  check(__clsll(0) == 63 && __clsll(-2) == 62, "clsll");
  __nop();
  __yield();
  __sev();
  __sevl();
  check(true, "hints");
  __dmb(11);
  __dsb(15);
  __isb(15);
  check(true, "barriers");
  uint32_t c = 0xffffffff;
  for (const char *p = "123456789"; *p; ++p)
    c = __crc32cb(c, (uint8_t)*p);
  check((c ^ 0xffffffff) == 0xe3069283, "crc32c check value");
  printf("ok %d\n", checks - fails);
  return fails;
}
