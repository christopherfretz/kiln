// [dcl.pre]/10, [dcl.typedef]/2: a header's block-scope aliases name the
// same types through a precompiled header, so a constant computed with
// them is the same and a wrong assertion about it still fails (WP-1320).
//@ expect: compile-fail
//@ diag: 8:15 error "static assertion failed due to requirement"
//@ diag: 8:33 note "expression evaluates to '8 == 3'"
#include "Inputs/local_decls.h"
static_assert(lcd::sizes<int>() == 3);
