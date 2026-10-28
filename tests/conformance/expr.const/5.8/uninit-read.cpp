// [expr.const]/5.11, [basic.indet]/2: lvalue-to-rvalue conversion of an indeterminate value.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_uninit_read`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 12:15 note "in call to 'f()'"
constexpr int f() {
  int x;
  return x;
}
static_assert(f() == 0);
