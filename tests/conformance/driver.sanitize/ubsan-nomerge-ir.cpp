// `-f[no-]sanitize-merge[=]` apply left to right: bare
// `-fno-sanitize-merge` clears the set, later spellings add and remove
// kinds; a handler call of a check outside the set carries `nomerge`.
// spec/codegen.md "Sanitizers", spec/driver.md, WP-1032.
//@ expect: ir
//@ std: c++20
//@ requires: clang>=22
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -O2 -Xclang -disable-llvm-passes -fno-sanitize-merge -fsanitize-merge=undefined,unsigned-integer-overflow -fno-sanitize-merge=vla-bound
#include "Inputs/ub-checks.h"
