// [cpp.include]: header-a.cpp with other text, and other expansions,
// before the `#include` (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: header-b.record
#define FIRST 1
#define SECOND (FIRST + FIRST)
int before = SECOND;
#include "Inputs/shared.h"
int b = SHARED(0);
