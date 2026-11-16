// [basic.life]/7, [class.union.general]/2, [expr.const]/10.9: a pointer to the
// inactive member `b` designates `b` after pointer arithmetic (an element
// of an array member, a decayed array member, one past the end and back)
// and after a round trip through memory (an array of pointers, a class
// member, a lambda capture, a returned pointer, a reference member,
// `std::addressof`), so reading through it while `a` (of the same type, at
// the same address) is active is rejected; `std::destroy_at` through it
// ends, and `std::construct_at` through it activates, that member (found
// running GCC's C++ testsuite; spec/oven.md "Union member designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 59:15 error "static assertion expression is not an integral constant expression"
//@ diag: 58:68 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 59:15 note "in call to 'indexed()'"
//@ diag: 61:15 error "static assertion expression is not an integral constant expression"
//@ diag: 60:59 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 61:15 note "in call to 'stored()'"
//@ diag: 63:15 error "static assertion expression is not an integral constant expression"
//@ diag: 62:66 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 63:15 note "in call to 'added()'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 64:76 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 65:15 note "in call to 'element()'"
//@ diag: 67:15 error "static assertion expression is not an integral constant expression"
//@ diag: 66:72 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 67:15 note "in call to 'decayed()'"
//@ diag: 69:15 error "static assertion expression is not an integral constant expression"
//@ diag: 68:84 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 69:15 note "in call to 'in_array()'"
//@ diag: 71:15 error "static assertion expression is not an integral constant expression"
//@ diag: 70:97 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 71:15 note "in call to 'captured()'"
//@ diag: 73:15 error "static assertion expression is not an integral constant expression"
//@ diag: 72:48 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 73:15 note "in call to 'returned()'"
//@ diag: 75:15 error "static assertion expression is not an integral constant expression"
//@ diag: 74:49 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 75:15 note "in call to 'addressof()'"
//@ diag: 77:15 error "static assertion expression is not an integral constant expression"
//@ diag: 76:60 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 77:15 note "in call to 'ref_member()'"
//@ diag: 79:15 error "static assertion expression is not an integral constant expression"
//@ diag: 78:94 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 79:15 note "in call to 'destroyed()'"
//@ diag: 81:15 error "static assertion expression is not an integral constant expression"
//@ diag: 80:67 note "destruction of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 81:15 note "in call to 'destroy_other()'"
//@ diag: 83:15 error "static assertion expression is not an integral constant expression"
//@ diag: 82:111 note "read of member 'a' of union with active member 'b' is not allowed in a constant expression"
//@ diag: 83:15 note "in call to 'constructed_other()'"
#include <memory>
union U { int a; int b; };
union A { int a[2]; int b[2]; };
struct Box { const int *p; };
struct R { const int &r; };
using I = int;
constexpr const int *ret(U &u) { return &u.b; }
constexpr int indexed() { U u{.a = 1}; const int *p = &u.b; return p[0]; }
static_assert(indexed() == 1);
constexpr int stored() { U u{.a = 1}; Box x{&u.b}; return *x.p; }
static_assert(stored() == 1);
constexpr int added() { U u{.a = 1}; const int *p = &u.b; return *(p + 1 - 1); }
static_assert(added() == 1);
constexpr int element() { A u{.a = {1, 2}}; const int *p = &u.b[1]; return *p; }
static_assert(element() == 2);
constexpr int decayed() { A u{.a = {1, 2}}; const int *p = u.b; return p[1]; }
static_assert(decayed() == 2);
constexpr int in_array() { U u{.a = 1}; const int *ps[2] = {nullptr, &u.b}; return *ps[1]; }
static_assert(in_array() == 1);
constexpr int captured() { U u{.a = 1}; const int *p = &u.b; auto f = [p] { return p; }; return *f(); }
static_assert(captured() == 1);
constexpr int returned() { U u{.a = 1}; return *ret(u); }
static_assert(returned() == 1);
constexpr int addressof() { U u{.a = 1}; return *std::addressof(u.b); }
static_assert(addressof() == 1);
constexpr int ref_member() { U u{.a = 1}; R x{u.b}; return x.r; }
static_assert(ref_member() == 1);
constexpr int destroyed() { U u{.a = 1}; int *ps[1] = {&u.a}; std::destroy_at(ps[0]); return u.b; }
static_assert(destroyed() == 1);
constexpr int destroy_other() { U u{.a = 1}; int *ps[1] = {&u.b}; (ps[0] + 0)->~I(); return 1; }
static_assert(destroy_other() == 1);
constexpr int constructed_other() { U u{.a = 1}; int *ps[1] = {&u.b}; std::construct_at(ps[0] + 0, 4); return u.a; }
static_assert(constructed_other() == 4);
// Accepted: the designated member is the active one.
constexpr int ok_indexed() { U u{.b = 1}; const int *p = &u.b; return p[0] + *(p + 1 - 1); }
static_assert(ok_indexed() == 2);
constexpr int ok_stored() { U u{.b = 1}; Box x{&u.b}; R y{u.b}; return *x.p + y.r; }
static_assert(ok_stored() == 2);
constexpr int ok_element() { A u{.b = {1, 2}}; const int *p = u.b; return p[1] + *std::addressof(u.b[0]); }
static_assert(ok_element() == 3);
constexpr int ok_constructed() { U u{.a = 1}; int *ps[1] = {&u.b}; std::construct_at(ps[0] + 0, 4); return u.b + *ps[0]; }
static_assert(ok_constructed() == 8);
constexpr int ok_destroyed() { U u{.a = 1}; int *ps[1] = {&u.a}; std::destroy_at(ps[0] + 0); std::construct_at(&u.b, 3); return u.b; }
static_assert(ok_destroyed() == 3);
constexpr int ok_past() { U u{.b = 1}; const int *p = &u.b + 1; return p[-1]; }
static_assert(ok_past() == 1);
