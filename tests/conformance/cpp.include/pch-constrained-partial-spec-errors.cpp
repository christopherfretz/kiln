// [temp.spec.partial.match]/2, [temp.constr.order]: `small<A>` alone
// selects the `A*` specialization the header constrains with a
// type-constraint; a wrong assertion about it still fails, as without a
// precompiled header (WP-1342).
//@ expect: compile-fail
//@ diag: 10:15 error "static assertion failed due to requirement"
//@ diag: 10:42 note "expression evaluates to '3 == 4'"
#include "Inputs/constrained_partial_specs.h"
struct O { using other = int; };
static_assert(cps::pick<int *, O>::value == 4);
