// C++17: `shift-base` checks signed `<<` too (C++20 defines it), with
// C++11's rule that a 1 may move into the sign bit.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: ir
//@ std: c++17
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -Xclang -disable-llvm-passes
#include "Inputs/ub-checks.h"
