// [ACLE] The SME state queries of <arm_sme.h> run on any AArch64 host:
// `__arm_has_sme()` and `__arm_in_streaming_mode()` read
// `__builtin_arm_get_sme_state`, a call of the SME ABI support routine
// `__arm_sme_state` (libgcc / compiler-rt), whose first result carries
// "SME available" in bit 63 and "streaming mode on" in bit 0. Outside a
// streaming function the streaming bit is clear whether or not the CPU has
// SME, and both helpers agree with the raw builtin (spec/codegen.md
// "Target builtins"). WP-1794. Host-restricted (no pinned `--target`): the
// header reaches <stdint.h>.
//@ requires: target-aarch64
//@ expect: run
//@ args: -march=armv9-a+sme2
//@ strict-diags
//@ stdout: ok 3
#include <arm_sme.h>
extern "C" int printf(const char *, ...);

static int checks;
static int fails;
static void check(bool c, const char *what) {
  ++checks;
  if (!c) {
    ++fails;
    printf("FAIL %s\n", what);
  }
}

__attribute__((noinline)) static bool streaming_compatible() __arm_streaming_compatible {
  return __arm_in_streaming_mode();
}

int main() {
  uint64_t x0 = 7;
  uint64_t x1 = 7;
  __builtin_arm_get_sme_state(&x0, &x1);
  bool has = __arm_has_sme();
  check(has == ((x0 >> 63) != 0), "__arm_has_sme agrees with bit 63");
  check((x0 & 1) == 0, "not streaming in main");
  check(!streaming_compatible(), "not streaming in a streaming-compatible callee");
  if (fails == 0)
    printf("ok %d\n", checks);
  return fails;
}
