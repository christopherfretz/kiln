// UBSan bool and enum: a loaded `bool` or enum outside its value range
// ([dcl.enum]/8, [basic.fundamental]/10) reports `load_invalid_value`.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.3, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=bool,enum -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-boolenum.h"
