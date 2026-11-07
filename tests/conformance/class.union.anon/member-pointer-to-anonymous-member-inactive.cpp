// [class.union.general]/6, [expr.const]/10.10: assigning through a
// pointer to a member of an anonymous union ([class.union.anon]/1) does
// not change the union's active member, so writing an inactive member
// that way is not a constant expression; the active one is.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:47 note "assignment to member 'l' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 18:15 note "in call to 'write_inactive()'"
struct E {
  char tag;
  union { int i; long l; };
};
constexpr int E::*pi = &E::i;
constexpr long E::*pl = &E::l;

constexpr int write_inactive() { E e{}; e.*pl = 7; return 0; }
static_assert(write_inactive() == 0, "");
constexpr int write_active() { E e{}; e.*pi = 7; return e.i; }
static_assert(write_active() == 7, "");
