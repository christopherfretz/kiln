// `-ftrapv` with `-fsanitize=signed-integer-overflow`: the sanitizer's
// handler call wins for the checks it makes; `-fwrapv` before `-ftrapv`
// is overridden (the arithmetic is still checked). spec/codegen.md
// "Sanitizers", WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -fwrapv -ftrapv -fsanitize=signed-integer-overflow,unsigned-integer-overflow -Wno-c11-extensions
#include "Inputs/trapv.h"
