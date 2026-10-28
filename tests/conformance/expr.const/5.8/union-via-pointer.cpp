// [class.union]/2: reading an inactive member through a pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_union_via_pointer`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:10 note "read of member 'f' of union with active member 'i' is not allowed in a constant expression"
//@ diag: 14:15 note "in call to 'f()'"
union U { int i; float f; };
constexpr int f() {
  U u = {1};
  float *q = &u.f;
  return *q;
}
static_assert(f() == 0);
