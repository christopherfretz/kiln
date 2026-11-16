// [class.copy.ctor]/15, [class.copy.assign]/13, [expr.const]/10.8-10.9: the
// implicit copy constructor and copy assignment of a union copy its object
// representation, not a member's value, so copying a union with no active
// member, or whose active member an explicit destruction ended, is a
// constant expression; the copy keeps that state, so reading the ended
// member through the copy is a read outside its lifetime and reading another
// member names the ended one as active. clang 22 and GCC 16.1 accept the
// copies; clang 22 rejects the reads as below; GCC 16.1 rejects the read of
// another member and the read with no active member, and reads the ended
// member's old value (spec/oven.md "Copying a union with an ended or no
// active member").
// The same in C++23, where an explicit destruction ends the member's bytes.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:84 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 44:15 note "in call to 'read_ended()'"
//@ diag: 46:15 error "static assertion expression is not an integral constant expression"
//@ diag: 45:84 note "read of member 'i' of union with active member 'm' is not allowed in a constant expression"
//@ diag: 46:15 note "in call to 'read_other()'"
//@ diag: 48:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:98 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 48:15 note "in call to 'read_assigned()'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 49:50 note "read of member 'a' of union with no active member is not allowed in a constant expression"
//@ diag: 50:15 note "in call to 'read_none()'"
//@ std: c++23
#include <memory>
struct S { int x; };
union U { S m; int i; };
union W { int a; int b; constexpr W() {} };
constexpr int copy_ended() { U u{.m = {1}}; std::destroy_at(&u.m); U v = u; return 1; }
static_assert(copy_ended() == 1);
constexpr int copy_none() { W u; W v = u; return 1; }
static_assert(copy_none() == 1);
constexpr int assign_ended() { U u{.m = {1}}; std::destroy_at(&u.m); U v{.i = 2}; v = u; return 1; }
static_assert(assign_ended() == 1);
constexpr int assign_none() { W u; W v; v.a = 3; v = u; return 1; }
static_assert(assign_none() == 1);
constexpr int copy_then_new() { U u{.m = {1}}; std::destroy_at(&u.m); U v = u; std::construct_at(&v.i, 4); return v.i; }
static_assert(copy_then_new() == 4);
// Rejected: the copy keeps the state.
constexpr int read_ended() { U u{.m = {1}}; std::destroy_at(&u.m); U v = u; return v.m.x; }
static_assert(read_ended() == 1);
constexpr int read_other() { U u{.m = {1}}; std::destroy_at(&u.m); U v = u; return v.i; }
static_assert(read_other() == 1);
constexpr int read_assigned() { U u{.m = {1}}; std::destroy_at(&u.m); U v{.i = 2}; v = u; return v.m.x; }
static_assert(read_assigned() == 1);
constexpr int read_none() { W u; W v = u; return v.a; }
static_assert(read_none() == 0);
