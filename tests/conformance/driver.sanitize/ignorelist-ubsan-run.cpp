// `-fsanitize-ignorelist=` under the UBSan runtime: overflow in a
// function named by `fun:` (a mangled name or a glob, in a section for
// the whole `undefined` group or for one check) or defined in a `src:`
// file is not instrumented, so it wraps silently; the same overflow in
// an unlisted function reports and, with `halt_on_error`, exits with the
// runtime's exit code. spec/codegen.md "Sanitizers", WP-1032.
//@ expect: run
//@ requires: sanitizer-runtime
//@ args: -fsanitize=signed-integer-overflow -fwrapv -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-ubsan.txt
//@ env: UBSAN_OPTIONS=halt_on_error=1:exitcode=7:print_summary=0
//@ exit: 7
//@ stdout: -2147483648 -2147483648 -2
//@ stdout: checked
#include "Inputs/ignorelist-src-ubsan.h"
#include <climits>
#include <cstdio>
#include <unistd.h>

int skipped_add(int a, int b) { return a + b; }
int skipped_mul(int a, int b) { return a * b; }
int checked_add(int a, int b) { return a + b; }

int main(int argc, char**) {
  std::printf("%d %d %d\n", skipped_add(INT_MAX, argc), header_add(INT_MAX, argc),
              skipped_mul(INT_MAX, 2 * argc));
  std::fflush(stdout);
  static const char msg[] = "checked\n";
  (void)!write(1, msg, sizeof msg - 1);
  return checked_add(INT_MAX, argc) == INT_MIN ? 0 : 1;
}
