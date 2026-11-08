// UBSan float-cast-overflow: a floating value outside the destination's range
// ([conv.fpint]/1), `_BitInt`, `_Atomic` and complex parts included.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.4, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=float-cast-overflow -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-floatcast.h"
