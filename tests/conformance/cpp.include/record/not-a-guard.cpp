// [cpp.include], [cpp.cond]: `b.h` is guarded by `B_H`, which `a.h` already
// defines. `b.h` is entered; its `#ifndef` is not taken, is not a guard
// kiln recognizes, and no definition comes from it (spec/pp.md
// "Preprocessing record", inclusions).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: not-a-guard.record
#include "Inputs/a.h"
#include "Inputs/b.h"
int b;
