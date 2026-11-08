// UBSan unreachable after a call that does not return, and nonnull-attribute
// and alignment on the arguments of memcpy/memmove/memset.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=unreachable,nonnull-attribute,alignment -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-noreturn.h"
