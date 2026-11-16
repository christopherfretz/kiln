// [basic.life]/7, [expr.const]/10.9: a pointer to the inactive member `b`
// designates `b` after pointer arithmetic and after a round trip through
// memory, so reading through it while `a` (of the same type, at the same
// address) is active is rejected. kiln's designator lives in the pointer
// register only (spec/oven.md "Union member designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:68 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 18:15 note "in call to 'indexed()'"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 19:59 note "read of member 'b' of union with active member 'a' is not allowed in a constant expression"
//@ diag: 20:15 note "in call to 'stored()'"
//@ xfail(kiln): WP-1508: the union member designator is dropped by pointer arithmetic and by a store of the pointer to memory
union U { int a; int b; };
struct Box { const int *p; };
constexpr int indexed() { U u{.a = 1}; const int *p = &u.b; return p[0]; }
static_assert(indexed() == 1);
constexpr int stored() { U u{.a = 1}; Box x{&u.b}; return *x.p; }
static_assert(stored() == 1);
