// `-fno-sanitize-merge=<kinds>` above -O0: those checks' traps get
// their own block and `nomerge`; the rest still share one trap block per
// handler (clang 20+ `-fsanitize-merge=`, default the `undefined`
// group). spec/codegen.md "Sanitizers", spec/driver.md, WP-1032.
//@ expect: ir
//@ std: c++20
//@ requires: clang>=22
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -fsanitize-trap=all -O2 -Xclang -disable-llvm-passes -fno-sanitize-merge=signed-integer-overflow,shift
#include "Inputs/ub-checks.h"
