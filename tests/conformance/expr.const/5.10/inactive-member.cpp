// [expr.const]/5.10: L2R of a non-active union member.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_10_inactive_member`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:10 note "read of member 'c' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
union U { int i; char c; };
constexpr char f() {
  U u = {65};
  return u.c;
}
static_assert(f() == 'A');
