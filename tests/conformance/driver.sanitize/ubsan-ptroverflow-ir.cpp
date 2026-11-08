// UBSan pointer-overflow: an inbounds GEP whose address computation wraps
// ([expr.add]/4).
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.5, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=pointer-overflow -Wno-vla-cxx-extension -Xclang -disable-llvm-passes
#include "Inputs/ub-ptroverflow.h"
