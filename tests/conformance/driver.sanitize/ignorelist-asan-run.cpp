// `-fsanitize-ignorelist=` under the ASan runtime: a heap read one past
// the end in a function named by `fun:` is not instrumented and goes
// unreported; the same read in an unlisted function is reported, and the
// runtime exits with its exit code. spec/codegen.md "Sanitizers",
// WP-1032.
//@ expect: run
//@ requires: sanitizer-runtime
//@ args: -fsanitize=address -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-asan-run.txt
//@ env: ASAN_OPTIONS=exitcode=9:detect_leaks=0:print_summary=0
//@ exit: 9
//@ stdout: skipped
#include <unistd.h>

char skipped_read(const char *p, int i) { return p[i]; }
char checked_read(const char *p, int i) { return p[i]; }

int main(int argc, char**) {
  char *p = new char[13]();
  volatile int i = 12 + argc;
  (void)skipped_read(p, i);
  static const char msg[] = "skipped\n";
  (void)!write(1, msg, sizeof msg - 1);
  (void)checked_read(p, i);
  delete[] p;
  return 0;
}
