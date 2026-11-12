// [dcl.fct]/5, [temp.deduct.call]/2: through a precompiled header, a
// header's dependent array parameter is still a pointer to the deduced
// element type, so a mismatched argument still fails (WP-1320).
//@ expect: compile-fail
//@ diag: 8:18 error "no matching function for call to 'first2'"
#include "Inputs/dependent_array_params.h"

int f() { return dap::first2(1); }
