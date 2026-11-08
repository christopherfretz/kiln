// `-fsanitize-trap=all` above -O0: one trap block per handler and
// function is shared (clang 22 keeps the `unsigned-*` checks' traps
// apart: they are outside the default `-fsanitize-merge=` set).
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -fsanitize-trap=all -O2 -Xclang -disable-llvm-passes
#include "Inputs/ub-checks.h"
