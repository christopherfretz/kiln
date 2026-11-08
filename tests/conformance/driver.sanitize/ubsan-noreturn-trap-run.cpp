// `-fsanitize=unreachable,nonnull-attribute -fsanitize-trap=all`: valid
// memcpy calls and calls that do not return run unchanged; a `[[noreturn]]`
// function that returns ([dcl.attr.noreturn]/2) traps at the check after its
// call instead of running off into whatever follows, with no sanitizer
// runtime linked.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.6, WP-871.
//@ expect: run
//@ exit: 3
//@ stdout: ok 7
//@ stdout: trapped
//@ args: -fsanitize=unreachable,nonnull-attribute -fsanitize-trap=all -Wno-invalid-noreturn
#include <csignal>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <unistd.h>

extern "C" void trapped(int) {
  static const char msg[] = "trapped\n";
  (void)!write(1, msg, sizeof msg - 1);
  _exit(3);
}

volatile int sink;
[[noreturn]] __attribute__((noinline)) void liar(int x) { sink = x; }
[[noreturn]] __attribute__((noinline)) void leave(int x) { std::exit(x); }

int main(int argc, char **) {
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  int a = 7, b = 0;
  std::memcpy(&b, &a, sizeof a);
  std::printf("ok %d\n", b);
  std::fflush(stdout);
  if (argc > 5) leave(4);
  liar(argc);
}
