// [dcl.attr.grammar]/5 `-fno-builtin-<name>` (clang
// `LangOpts.NoBuiltinFuncs`): every function and call site carries
// `"no-builtin-<name>"`, and a call of `<name>` is `nobuiltin` and not
// lowered; the other library builtins still lower. `std-move` names
// `std::move`. spec/codegen.md "no-builtin", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized -fno-builtin-memset -fno-builtin-std-move -fno-builtin-memset
#include "Inputs/no-builtin.h"
