// UBSan nonnull-attribute, returns-nonnull-attribute and builtin: a null
// argument or return of a `nonnull`/`returns_nonnull` function, a zero
// operand of `__builtin_ctz`/`clz`.
// Trap mode (`-fsanitize-trap=all`): each failing check `llvm.ubsantrap`s.
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2.6, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=nonnull-attribute,returns-nonnull-attribute,builtin -Wno-nonnull -Wno-ignored-attributes -fsanitize-trap=all -Xclang -disable-llvm-passes
#include "Inputs/ub-nonnull.h"
