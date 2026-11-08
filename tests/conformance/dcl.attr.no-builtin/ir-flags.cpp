// [dcl.attr.grammar]/5 `-fno-builtin` (clang `LangOpts.NoBuiltin`): every
// function is `"no-builtins"`, every call site `nobuiltin
// "no-builtins"`, direct or indirect, a constructor, a destructor or a
// virtual call; no library function is lowered as its builtin and
// `std::move` is an ordinary call; `__builtin_*` still lower. A
// `no_builtin` attribute adds nothing. spec/codegen.md "no-builtin",
// WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized -fno-builtin
#include "Inputs/no-builtin.h"
