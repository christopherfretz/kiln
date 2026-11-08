// UBSan unreachable after a call that does not return, and nonnull-attribute
// and alignment on the arguments of memcpy/memmove/memset.
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=unreachable,nonnull-attribute,alignment -Xclang -disable-llvm-passes
#include "Inputs/ub-noreturn.h"
