// [class.access]/4, [over.match.best]/3, [class.member.lookup]: through
// a precompiled header, an overloaded member access still chooses the
// overload the source does (the failed assertions show the value), still
// checks the access of the overload it chooses and still finds an
// ambiguity among the members a using-declaration joins (WP-1368; the
// last two are diagnosed in the header, compared under `--pch-diff`).
//@ expect: compile-fail
//@ diag: 13:15 error "static assertion failed due to requirement 'om::used(1L) == 1'"
//@ diag: 13:28 note "expression evaluates to '3 == 1'"
//@ diag: 14:15 error "static assertion failed due to requirement 'om::hid(1) == 1'"
//@ diag: 14:26 note "expression evaluates to '2 == 1'"
#include "Inputs/ovl_members.h"
static_assert(om::used(1L) == 1);
static_assert(om::hid(1) == 1);
int a = om::callp(1);
int b = om::used(1u);
