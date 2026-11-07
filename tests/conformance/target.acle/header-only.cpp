// [driver] `<arm_acle.h>` parses with every feature its inline wrappers
// reach: the `__builtin_arm_*` builtins they call are declared on AArch64,
// and `__has_builtin` answers clang 19's 0 for the hint and barrier names
// clang 22 declares in its own header (`__yield`, `__dmb` ...), so the shipped
// clang 19 header defines them (spec/landed/sema/WP-724.md "ACLE
// builtins"; spec/pp.md `__has_builtin`). WP-724.
//@ requires: target-aarch64
//@ strict-diags
//@ args: -march=armv8.5-a+crc+crypto+rng+memtag+ls64
#include <arm_acle.h>

void f() {
  __yield();
  __wfe();
  __dmb(11);
}

int main() { return 0; }
