// UBSan bool and enum: a loaded `bool` or enum outside its value range
// ([dcl.enum]/8, [basic.fundamental]/10) reports `load_invalid_value`.
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.3, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=bool,enum -Xclang -disable-llvm-passes
#include "Inputs/ub-boolenum.h"
