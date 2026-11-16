// [expr.add]/5: pointers to two members of one union (or to elements of
// two array members) are not elements of one array object, so subtracting
// them is not a constant expression, as clang says (GCC 16.1 accepts);
// [expr.eq], [expr.rel]/5: comparing them compares addresses (equal), and
// subtraction within one member stays valid (spec/oven.md "Union member
// designators").
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:56 note "subtracted pointers are not elements of the same array"
//@ diag: 21:15 note "in call to 'subtracted()'"
//@ diag: 23:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:73 note "subtracted pointers are not elements of the same array"
//@ diag: 23:15 note "in call to 'subtracted_elements()'"
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:97 note "subtracted pointers are not elements of the same array"
//@ diag: 25:15 note "in call to 'subtracted_stored()'"
union U { int a; int b; };
union A { int a[2]; int b[2]; };
constexpr long subtracted() { U u{.a = 1}; return &u.b - &u.a; }
static_assert(subtracted() == 0);
constexpr long subtracted_elements() { A u{.a = {1, 2}}; return &u.b[1] - &u.a[0]; }
static_assert(subtracted_elements() == 1);
constexpr long subtracted_stored() { U u{.a = 1}; const int *ps[2] = {&u.a, &u.b}; return ps[1] - ps[0]; }
static_assert(subtracted_stored() == 0);
// Accepted: equality and the relational operators compare addresses
// ([expr.eq], [expr.rel]/5), and subtraction within one member is fine.
constexpr bool equal() { U u{.a = 1}; return &u.a == &u.b && &u.b + 1 == &u.a + 1; }
static_assert(equal());
constexpr bool related() { U u{.a = 1}; return !(&u.a < &u.b) && &u.a <= &u.b && &u.b >= &u.a; }
static_assert(related());
constexpr long same_member() { A u{.a = {1, 2}}; return &u.b[1] - &u.b[0] + (&u.a[1] - u.a); }
static_assert(same_member() == 2);
constexpr long plain(const int *p, const int *q) { return p - q; }
constexpr long same_member_call() { A u{.a = {1, 2}}; return plain(u.b + 2, &u.b[0]); }
static_assert(same_member_call() == 2);
