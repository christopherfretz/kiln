// [dcl.attr.grammar]/5 `-fbuiltin` cancels an earlier `-fno-builtin`,
// `-fhosted` an earlier `-ffreestanding`: the library builtins lower
// again. spec/codegen.md "no-builtin", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized -ffreestanding -fno-builtin -fbuiltin -fhosted
#include "Inputs/no-builtin.h"
