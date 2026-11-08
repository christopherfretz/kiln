// UBSan pointer-overflow: an inbounds GEP whose address computation wraps
// ([expr.add]/4).
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.5, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=pointer-overflow -Wno-vla-cxx-extension -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-ptroverflow.h"
