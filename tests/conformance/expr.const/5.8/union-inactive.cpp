// [class.union]/2, [expr.const]/5.10: reading an inactive union member (type punning).
// Mirrors crates/kiln_oven/tests/ub.rs `ub_union_inactive`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:10 note "read of member 'f' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
union U { int i; float f; };
constexpr float f() {
  U u = {1};
  return u.f;
}
static_assert(f() == 0);
