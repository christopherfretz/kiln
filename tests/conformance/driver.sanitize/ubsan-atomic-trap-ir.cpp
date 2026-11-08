// UBSan the overflow checks on `_Atomic` `++`/`--` and compound assignment:
// an unsigned operand takes the compare-exchange loop with the checked
// operation, a signed one stays an unchecked `atomicrmw`.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.8, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=signed-integer-overflow,unsigned-integer-overflow,shift,integer-divide-by-zero -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-atomic.h"
