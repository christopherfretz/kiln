// UBSan null, alignment and object-size: one combined `EmitTypeCheck` per
// access with clang's type-check kind, alignment and object-size operands
// (no check of a declaration reference, an implicit-move return or a
// known-aligned alloca; operator-call checks at clang's `getExprLoc`).
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.1, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=null,alignment,object-size -O2 -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-typecheck.h"
