// [basic.life]/1, [class.dtor]/19, [class.union.general]/2, [expr.const]/10.8:
// a member of a union that is not its active member is not within its
// lifetime, so invoking its destructor is undefined and not a constant
// expression, through the member access or a pointer stored to memory:
// "destruction of member 'i' of union with active member 'm'", as clang 22
// says in every language mode (GCC 16.1 rejects it too). Reading another
// member through a pointer after the active member was destroyed names the
// destroyed member as active. Destroying the active member and
// constructing another is accepted (spec/oven.md "Union member
// designators").
// The same in C++23, where an explicit destruction ends the member's bytes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:43 note "destruction of member 'i' of union with active member 'm' is not allowed in a constant expression"
//@ diag: 34:15 note "in call to 'inactive()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:67 note "destruction of member 'i' of union with active member 'd' is not allowed in a constant expression"
//@ diag: 36:15 note "in call to 'inactive_class()'"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 37:71 note "destruction of member 'i' of union with active member 'm' is not allowed in a constant expression"
//@ diag: 38:15 note "in call to 'inactive_stored()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 39:100 note "read of member 'i' of union with active member 'm' is not allowed in a constant expression"
//@ diag: 40:15 note "in call to 'read_through_pointer()'"
//@ std: c++23
#include <memory>
struct S { int x; };
struct D { int x; constexpr ~D() {} };
union U { S m; int i; long l; };
union V { D d; int i; constexpr V() : i() {} constexpr ~V() {} };
using I = int;
constexpr int inactive() { U u{.m = {1}}; u.i.~I(); return 1; }
static_assert(inactive() == 1);
constexpr int inactive_class() { V v; std::construct_at(&v.d, 1); v.i.~I(); return 1; }
static_assert(inactive_class() == 1);
constexpr int inactive_stored() { U u{.m = {1}}; int *ps[1] = {&u.i}; ps[0]->~I(); return 1; }
static_assert(inactive_stored() == 1);
constexpr int read_through_pointer() { U u{.m = {1}}; std::destroy_at(&u.m); int *p = &u.i; return *p; }
static_assert(read_through_pointer() == 1);
// Accepted: destroying the active member, then constructing another.
constexpr int active() { U u{.m = {1}}; std::destroy_at(&u.m); std::construct_at(&u.i, 2); return u.i; }
static_assert(active() == 2);
constexpr int active_class() { V v; std::construct_at(&v.d, 1); std::destroy_at(&v.d); std::construct_at(&v.i, 3); return v.i; }
static_assert(active_class() == 3);
