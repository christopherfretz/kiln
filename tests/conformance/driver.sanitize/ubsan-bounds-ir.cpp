// UBSan array-bounds: the index against the constant or VLA bound of the
// accessed array (flexible array members and vectors as clang).
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.2, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=array-bounds -Wno-vla-cxx-extension -Wno-zero-length-array -Xclang -disable-llvm-passes
#include "Inputs/ub-bounds.h"
