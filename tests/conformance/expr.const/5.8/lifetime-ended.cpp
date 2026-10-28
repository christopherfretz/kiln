// [basic.life]/7, [expr.const]/5.8: reading a variable after its lifetime ended.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_lifetime_ended`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14:10 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 16:15 note "in call to 'f()'"
constexpr int f() {
  int *p = nullptr;
  {
    int x = 1;
    p = &x;
  }
  return *p;
}
static_assert(f() == 1);
