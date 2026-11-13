// [class.access]/4, [over.match.best]/3, [class.member.lookup],
// [module.interface], spec/modules.md §11.11 "Forms carried by
// WP-1368": an imported module's overloaded member access still chooses
// the overload the source does (the failed assertions show the value),
// still checks the access of the overload it chooses and still finds an
// ambiguity among the members a using-declaration joins, with the
// candidate sets built from the declaration image.
//@ expect: compile-fail
//@ std: c++26
//@ module: imgovl=Inputs/img-ovl-members.cppm
//@ diag: 16:15 error "static assertion failed due to requirement 'om::used(1L) == 1'"
//@ diag: 16:28 note "expression evaluates to '3 == 1'"
//@ diag: 17:15 error "static assertion failed due to requirement 'om::hid(1) == 1'"
//@ diag: 17:26 note "expression evaluates to '2 == 1'"
import imgovl;
static_assert(om::used(1L) == 1);
static_assert(om::hid(1) == 1);
int a = om::callp(1);
int b = om::used(1u);
