// `-ftrapv`: signed `+ - *` and negation that overflow execute the trap
// instruction (SIGTRAP on AArch64, SIGILL on x86-64) with no sanitizer
// runtime; unsigned arithmetic wraps and in-range values pass through.
// [expr.pre]/4, spec/codegen.md "Sanitizers", WP-1032.
//@ expect: run
//@ exit: 3
//@ args: -ftrapv
//@ stdout: 42 -6 -12 2 4294967295
//@ stdout: -2147483647
//@ stdout: trapped
#include <climits>
#include <csignal>
#include <cstdio>
#include <unistd.h>

extern "C" void trapped(int) {
  static const char msg[] = "trapped\n";
  (void)!write(1, msg, sizeof msg - 1);
  _exit(3);
}

int add(int a, int b) { return a + b; }
int sub(int a, int b) { return a - b; }
int mul(int a, int b) { return a * b; }
int neg(int a) { return -a; }
unsigned usub(unsigned a, unsigned b) { return a - b; }

int main(int argc, char**) {
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  std::printf("%d %d %d %d %u\n", add(40, 2), sub(-7, -1), mul(-3, 4), neg(-2), usub(0u, 1u));
  std::printf("%d\n", neg(INT_MAX));
  std::fflush(stdout);
  std::printf("%d\n", neg(INT_MIN + argc - 1));
  return 0;
}
