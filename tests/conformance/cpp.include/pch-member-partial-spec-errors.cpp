// [temp.spec.partial.general]/7, [temp.spec.partial.match]: a nested
// class's member template selects its partial specialization; a wrong
// assertion about it still fails, as without a precompiled header
// (WP-1342).
//@ expect: compile-fail
//@ diag: 9:15 error "static assertion failed due to requirement"
//@ diag: 9:50 note "expression evaluates to '3 == 0'"
#include "Inputs/member_partial_specs.h"
static_assert(mps::outer::inner::arr<char, 3>::n == 0);
