// `-fsanitize=integer`: the implicit-integer-* conversion checks are in the
// group, implicit-bitfield-conversion is not (clang `SanitizerKind::Integer`).
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.7, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=integer -Xclang -disable-llvm-passes
#include "Inputs/ub-implicit-conv.h"
