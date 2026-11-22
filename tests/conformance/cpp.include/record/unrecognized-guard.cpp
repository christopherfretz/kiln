// [cpp.include]: a guard kiln does not recognize (a declaration after the
// `#endif`). The header is entered twice, in two contexts; the second
// time its `#ifndef` is not taken and its group spans the body, so a client
// sees that the body was skipped, not read and found empty (spec/pp.md
// "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: unrecognized-guard.record
#include "Inputs/leaky.h"
#include "Inputs/leaky.h"
