// UBSan implicit-integer-truncation, implicit-integer-sign-change and
// implicit-bitfield-conversion: an implicit integral conversion
// ([conv.integral]/3) or bit-field store that changes the value.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.7, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=implicit-conversion -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-implicit-conv.h"
