// Fatal and recoverable checks mixed: `-fno-sanitize-recover=` checks
// call the `_abort` handlers; one operation checked both ways branches
// on the fatal condition first.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -fsanitize-recover=signed-integer-overflow -fno-sanitize-recover=shift,integer-divide-by-zero,unsigned-integer-overflow -Xclang -disable-llvm-passes
#include "Inputs/ub-checks.h"
