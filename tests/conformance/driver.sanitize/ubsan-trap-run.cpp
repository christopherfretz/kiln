// `-fsanitize-trap=all`: a UBSan check that fails executes the trap
// instruction (SIGTRAP on AArch64's `brk`, SIGILL on x86-64's `ud2`) and
// links no sanitizer runtime; checks that pass leave the values alone.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: run
//@ exit: 3
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,unsigned-integer-overflow -fsanitize-trap=all
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
int dv(int a, int b) { return a / b; }
int shl(int a, int b) { return a << b; }
unsigned usub(unsigned a, unsigned b) { return a - b; }

int main(int argc, char**) {
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  std::printf("%d %d %d %u\n", add(40, 2), dv(-7, 2), shl(1, 30), usub(5u, 3u));
  std::printf("%d\n", dv(INT_MIN, -argc + 2));
  std::fflush(stdout);
  std::printf("%d\n", add(INT_MAX, argc));
  return 0;
}
