// `-ftrapv -ftrapv-handler=name`: an overflow calls
// `i64 name(i64 lhs, i64 rhs, i8 op, i8 width, ...)` in a block at the
// function's end (op is `(add 1 | sub 2 | mul 3) << 1 | signed`, the
// operands sign-extended) and its truncated result is the value, merged
// by a `phi` -- for the sanitizer's checks too (clang
// `EmitOverflowCheckedBinOp`). spec/codegen.md "Sanitizers", WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -ftrapv -ftrapv-handler=overflow_handler -fsanitize=unsigned-integer-overflow -Wno-c11-extensions
#include "Inputs/trapv.h"
int branchy(int a, int b, int c) {
  int x = a + b;
  if (c)
    x = x * c;
  return x - 1;
}
