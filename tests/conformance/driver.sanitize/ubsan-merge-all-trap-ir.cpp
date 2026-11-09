// Bare `-fsanitize-merge` is `=all`: the `unsigned-*` checks, outside
// the default set, share their trap blocks too. spec/codegen.md
// "Sanitizers", spec/driver.md, WP-1032.
//@ expect: ir
//@ std: c++20
//@ requires: clang>=22
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -fsanitize-trap=all -O2 -Xclang -disable-llvm-passes -fsanitize-merge
#include "Inputs/ub-checks.h"
