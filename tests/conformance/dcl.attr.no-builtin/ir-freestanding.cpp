// [intro.compliance.general]/7 [dcl.attr.grammar]/5 `-ffreestanding`
// implies `-fno-builtin` (and no `uwtable`); `-fbuiltin` after it does
// not cancel it. spec/codegen.md "no-builtin", WP-808.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized -ffreestanding -fbuiltin
#include "Inputs/no-builtin.h"
