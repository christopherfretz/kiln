// UBSan implicit-integer-truncation, implicit-integer-sign-change and
// implicit-bitfield-conversion: an implicit integral conversion
// ([conv.integral]/3) or bit-field store that changes the value.
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.7, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=implicit-conversion -Xclang -disable-llvm-passes
#include "Inputs/ub-implicit-conv.h"
