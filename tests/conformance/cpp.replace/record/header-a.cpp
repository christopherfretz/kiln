// [cpp.include]: a header's entries have the same keys whatever precedes
// it (header-b.cpp has other text before the `#include`): every `shared.h`
// line of the two dumps is equal (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: header-a.record
#include "Inputs/shared.h"
int a = SHARED(0);
