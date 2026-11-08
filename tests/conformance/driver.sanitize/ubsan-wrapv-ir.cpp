// `-fwrapv`: signed arithmetic is still checked; an elided check keeps
// `nsw` (clang's overflow-behavior fallthrough).
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -fwrapv -Xclang -disable-llvm-passes
#include "Inputs/ub-checks.h"
