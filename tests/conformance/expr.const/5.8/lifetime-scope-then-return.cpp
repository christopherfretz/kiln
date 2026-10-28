// [basic.life]/7: a block-scope local escaping its scope and its call.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_lifetime_scope_then_return`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:28 note "read of variable whose lifetime has ended"
//@ diag: 18:15 note "in call to 'f()'"
//@ diag: 12:9 note "declared here"
constexpr int *g() {
  int *p = nullptr;
  {
    int x = 1;
    p = &x;
  }
  return p;
}
constexpr int f() { return *g(); }
static_assert(f() == 1);
