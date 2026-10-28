// [basic.life]/7: a dangling pointer to a callee's local.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_lifetime_returned_local`.
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-return-stack-address
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:28 note "read of variable whose lifetime has ended"
//@ diag: 15:15 note "in call to 'f()'"
//@ diag: 11:7 note "declared here"
constexpr int *g() {
  int x = 1;
  return &x;
}
constexpr int f() { return *g(); }
static_assert(f() == 1);
