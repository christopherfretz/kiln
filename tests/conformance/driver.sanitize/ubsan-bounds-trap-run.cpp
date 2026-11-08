// `-fsanitize=array-bounds,null,alignment -fsanitize-trap=all`: in-bounds
// subscripts and valid accesses run unchanged; the first out-of-bounds
// subscript of a constant-bound array ([expr.sub]/2, [expr.add]/4) traps
// with no sanitizer runtime linked.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.2, WP-871.
//@ expect: run
//@ exit: 3
//@ args: -fsanitize=array-bounds,null,alignment -fsanitize-trap=all
#include <csignal>
#include <cstdio>
#include <unistd.h>

extern "C" void trapped(int) {
  static const char msg[] = "trapped\n";
  (void)!write(1, msg, sizeof msg - 1);
  _exit(3);
}

int g[4] = {1, 2, 3, 4};
struct S { int a[2]; int fam[1]; };
int at(int i) { return g[i]; }
int deref(int *p) { return *p; }
int fam(S *s, int i) { return s->fam[i]; }

int main(int argc, char**) {
  std::signal(SIGTRAP, trapped);
  std::signal(SIGILL, trapped);
  S s = {{5, 6}, {7}};
  std::printf("%d %d %d\n", at(0) + at(3), deref(&g[argc]), fam(&s, 0));
  std::fflush(stdout);
  std::printf("%d\n", at(argc + 3));
  return 0;
}
