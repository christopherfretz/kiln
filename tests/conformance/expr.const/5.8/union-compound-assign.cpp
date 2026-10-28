// [class.union]/2: `u.f += 1` reads the inactive member.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_union_compound_assign`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:7 note "assignment to member 'f' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 14:15 note "in call to 'f()'"
union U { int i; float f; };
constexpr int f() {
  U u = {1};
  u.f += 1;
  return 0;
}
static_assert(f() == 0);
