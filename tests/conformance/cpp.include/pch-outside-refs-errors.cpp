// [class.mem.general]/7, [class.pre]/2, [temp.constr.constr]: through a
// precompiled header, a member named before its declaration is still
// the class's, a self-based class still converts to its base only, and
// a concept's requires-expression is still checked (WP-1360).
//@ expect: compile-fail
//@ diag: 11:13 error "cannot initialize a member subobject of type 'int' with an lvalue of type 'const char[2]'"
//@ diag: 13:9 error "no matching function for call to 'need'"
//@ diag: 12:31 note "constraints not satisfied [with T = o3::cmp]"
//@ diag: 14:14 error "cannot initialize a variable of type 'o2::derived *' with an rvalue of type 'o2::base<o2::both> *'"
#include "Inputs/outside_refs.h"
o1::plain p{"x"};
template <o4::testable T> int need(T) { return 0; }
int y = need(o3::cmp{});
o2::derived *d = (o2::base<o2::both> *)nullptr;
