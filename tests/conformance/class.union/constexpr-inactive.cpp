// [class.union]/2, /6, [expr.const]/10.9: reading a member of a union other
// than its active member (or of a union with no active member), directly,
// through an anonymous union or through a pointer to member, is not a
// constant expression; an assignment through a pointer to member does not
// change the active member ([class.union]/6 names only member access).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:46 note "read of member 'i' of union with active member 'f' is not allowed in a constant expression"
//@ diag: 25:15 note "in call to 'inactive()'"
//@ diag: 28:15 error "static assertion expression is not an integral constant expression"
//@ diag: 27:52 note "read of member 'p' of union with active member 'q' is not allowed in a constant expression"
//@ diag: 28:15 note "in call to 'anonymous()'"
//@ diag: 31:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:74 note "read of member 'y' of union with active member 'x' is not allowed in a constant expression"
//@ diag: 31:15 note "in call to 'via_member_pointer()'"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:36 note "read of member 'x' of union with no active member is not allowed in a constant expression"
//@ diag: 34:15 note "in call to 'none()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 35:79 note "assignment to member 'y' of union with active member 'x' is not allowed in a constant expression"
//@ diag: 36:15 note "in call to 'assign_via_member_pointer()'"
union U { int i; float f; constexpr U() : i(1) {} constexpr U(float x) : f(x) {} };
constexpr int inactive() { U u(2.0f); return u.i; }
static_assert(inactive() == 0);
struct W { union { int p; char q; }; int k; constexpr W() : p(5), k(1) {} };
constexpr int anonymous() { W w; w.q = 'a'; return w.p; }
static_assert(anonymous() == 0);
union P { int x; long y; };
constexpr long via_member_pointer() { P u{1}; long P::*m = &P::y; return u.*m; }
static_assert(via_member_pointer() == 0);
union E { int x; char c; constexpr E() {} };
constexpr int none() { E e; return e.x; }
static_assert(none() == 0);
constexpr long assign_via_member_pointer() { P u{1}; long P::*m = &P::y; u.*m = 6; return u.y; }
static_assert(assign_via_member_pointer() == 6);
