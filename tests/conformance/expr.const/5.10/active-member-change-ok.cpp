// [expr.const]/5.10, [class.union]/6: assignment changes the active member.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_10_active_member_change_ok`.
union U { int i; float f; };
constexpr float f() {
  U u = {1};
  u.f = 2.5f;
  return u.f;
}
static_assert(f() == 2.5f);
