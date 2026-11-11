// [dcl.enum]/5, [temp.inst]/3, [dcl.pre]/12: an enumerator of a class
// template in an included header has the value its instantiation gives
// it, and an initializer invalid for the arguments is diagnosed at the
// instantiation, as without a precompiled header (WP-1310).
//@ expect: compile-fail
//@ diag: 10:15 error "static assertion failed due to requirement 'me::A<char>::d == 5'"
//@ diag: 10:30 note "expression evaluates to '4 == 5'"
#include "Inputs/member_enums.h"

static_assert(me::A<char>::d == 5);
me::B<int> k;
