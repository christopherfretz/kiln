// UBSan checks that report and continue (the default): each check's
// `i1` branches (likely) to a `__ubsan_handle_*` call with the source
// location and type descriptors; `!nosanitize` on every check
// instruction; elided checks (constants, promoted narrow operands).
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.4, WP-675.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,integer-divide-by-zero,shift,return,unreachable,vla-bound,unsigned-integer-overflow,unsigned-shift-base -Wno-vla-cxx-extension -Xclang -disable-llvm-passes
#include "Inputs/ub-checks.h"
