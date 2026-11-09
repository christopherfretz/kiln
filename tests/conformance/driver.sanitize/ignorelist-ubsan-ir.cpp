// `-fsanitize-ignorelist=` for UBSan: a function named by `fun:` or
// defined in a `src:` file loses the checks of the section that names it
// (the `[undefined]` group, or one check), so `skip` and the header's
// `header_add` have no checks, `only_sio` keeps its shift check, and
// `shift_off` keeps its overflow check (clang `isInNoSanitizeList` in
// `StartFunction`). spec/codegen.md "Sanitizers", WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,shift -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-ubsan-ir.txt
#include "Inputs/ignorelist-src-ubsan.h"

int skip(int a, int b) { return (a + b) << b; }
int only_sio(int a, int b) { return (a + b) << b; }
int shift_off(int a, int b) { return (a + b) << b; }
int kept(int a, int b) { return (a + b) << b; }
int use(int a) { return header_add(a, 1); }
