// [temp.spec.partial.match]/2, [temp.mem]/1, [temp.mem.func]/1: wrong
// assertions about the specializations the header's snapshot
// carries (WP-1358) still fail, as without a precompiled header.
//@ expect: compile-fail
//@ diag: 12:15 error "static assertion failed due to requirement"
//@ diag: 12:34 note "expression evaluates to '2 == 3'"
//@ diag: 13:15 error "static assertion failed due to requirement"
//@ diag: 14:15 error "static assertion failed due to requirement"
//@ diag: 14:31 note "expression evaluates to '3 == 1'"
//@ std: c++20
#include "Inputs/spec_forms.h"
static_assert(sf::A<double *>::k == 3);
static_assert(!sf::has<int>::value);
static_assert(sf::k_v<long *> == 1);
