// UBSan float-cast-overflow: a floating value outside the destination's range
// ([conv.fpint]/1), `_BitInt`, `_Atomic` and complex parts included.
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.4, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=float-cast-overflow -Xclang -disable-llvm-passes
#include "Inputs/ub-floatcast.h"
