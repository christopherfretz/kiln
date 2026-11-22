// [cpp.include]: one header entered twice under different include
// contexts, directly and through `wrap.h`. Each instance has its own rows;
// the rows of the one inside `wrap.h` name it as their context (spec/pp.md
// "Preprocessing record", include contexts).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: two-contexts.record
#define CTX direct
#include "Inputs/ctx.h"
#undef CTX
#include "Inputs/wrap.h"
