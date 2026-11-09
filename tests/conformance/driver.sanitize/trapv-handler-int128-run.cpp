// `-ftrapv -ftrapv-handler=name` on `__int128`: the handler's
// `(i64, i64, i8, i8, ...)` signature cannot carry 128-bit operands
// (clang `EmitOverflowCheckedBinOp` emits an invalid `sext i128 to i64`
// there, and clang 19 and 22 crash in the backend), so kiln keeps the
// plain `-ftrapv` trap for a 128-bit overflow instead of narrowing the
// operands. In-range 128-bit arithmetic keeps every bit, and a 32-bit
// overflow still calls the handler. (No 128-bit `*`: its overflow
// intrinsic needs compiler-rt's `__muloti4`, absent from libgcc.) [expr.pre]/4, spec/codegen.md
// "Sanitizers", WP-1044.
//@ expect: run
//@ skip(clang): clang 19 and 22 crash on -ftrapv-handler= with __int128 arithmetic
//@ args: -ftrapv -ftrapv-handler=overflow_handler
//@ exit: 3
//@ stdout: 7fffffffffffffff ffffffffffffffff
//@ stdout: 8000000000000000 0000000000000000
//@ stdout: 0000000000000000 0000000000000001
//@ stdout: handler 2147483647 1 op=3 width=32
//@ stdout: 7
//@ stdout: trapped
#include <csignal>
#include <cstdio>
#include <unistd.h>

extern "C" long overflow_handler(long a, long b, char op, unsigned char width, ...) {
  std::printf("handler %ld %ld op=%d width=%d\n", a, b, op, width);
  return 7;
}

extern "C" void trapped(int) {
  static const char msg[] = "trapped\n";
  (void)!write(1, msg, sizeof msg - 1);
  _exit(3);
}

using i128 = __int128;
using u128 = unsigned __int128;

i128 add(i128 a, i128 b) { return a + b; }
i128 sub(i128 a, i128 b) { return a - b; }
int add32(int a, int b) { return a + b; }

void show(i128 v) {
  u128 u = static_cast<u128>(v);
  std::printf("%016llx %016llx\n", static_cast<unsigned long long>(u >> 64),
              static_cast<unsigned long long>(u));
}

int main() {
  const i128 max = static_cast<i128>(~static_cast<u128>(0) >> 1);
  const i128 hi = static_cast<i128>(1) << 64;
  // In range: no operand bit is lost.
  show(sub(add(max, -hi), -hi));
  show(sub(-max, 1));
  show(add(hi >> 64, 0));
  std::printf("%d\n", add32(2147483647, 1));
  std::fflush(stdout);
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  show(add(max, 1));
  return 0;
}
