// `-fsanitize=implicit-conversion,float-cast-overflow -fsanitize-trap=all`:
// value-preserving conversions run unchanged; an implicit truncation that
// changes the value ([conv.integral]/3) traps with no runtime linked.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.7, WP-871.
//@ expect: run
//@ exit: 3
//@ args: -fsanitize=implicit-conversion,float-cast-overflow -fsanitize-trap=all
#include <csignal>
#include <cstdio>
#include <unistd.h>

extern "C" void trapped(int) {
  static const char msg[] = "trapped\n";
  (void)!write(1, msg, sizeof msg - 1);
  _exit(3);
}

struct B { int b : 3; unsigned c : 4; };
unsigned char narrow(int x) { return x; }
int fl(double d) { return d; }
void setb(B &s, int v) { s.b = v; }

int main(int argc, char**) {
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  B s = {};
  setb(s, -4);
  std::printf("%d %d %d\n", narrow(200 + argc), fl(1.5e9), s.b);
  std::fflush(stdout);
  std::printf("%d\n", narrow(255 + argc));
  return 0;
}
