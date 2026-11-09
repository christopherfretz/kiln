// UBSan array-bounds: the index against the constant or VLA bound of the
// accessed array (flexible array members and vectors as clang).
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.2, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=array-bounds -Wno-vla-cxx-extension -Wno-zero-length-array -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-bounds.h"
