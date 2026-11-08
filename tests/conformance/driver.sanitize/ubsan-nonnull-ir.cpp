// UBSan nonnull-attribute, returns-nonnull-attribute and builtin: a null
// argument or return of a `nonnull`/`returns_nonnull` function, a zero
// operand of `__builtin_ctz`/`clz`.
// Recover mode: each failing check calls its `__ubsan_handle_*` handler.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §1.5.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=nonnull-attribute,returns-nonnull-attribute,builtin -Wno-nonnull -Wno-ignored-attributes -Xclang -disable-llvm-passes
#include "Inputs/ub-nonnull.h"
