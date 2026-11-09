// `-ftrapv`: signed `+ - *`, negation and `++`/`--` that may overflow
// trap through `llvm.{s}{add,sub,mul}.with.overflow` and a bare
// `llvm.ubsantrap` (clang `EmitTrapCheck`: no check data, no runtime;
// a negation traps as the subtraction 0 - x). Unsigned, division and
// shifts are left alone, and an `_Atomic` integer takes the
// compare-exchange loop. [expr.pre]/4, spec/codegen.md "Sanitizers",
// WP-1032.
//@ expect: ir
//@ std: c++20
//@ args: -ftrapv -Wno-c11-extensions
#include "Inputs/trapv.h"
