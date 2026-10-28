// [expr.call]/1: calling a null function pointer.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_null_fn_ptr`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:10 note "'fp' evaluates to a null function pointer"
//@ diag: 12:15 note "in call to 'f()'"
constexpr int f() {
  int (*fp)(int) = nullptr;
  return fp(1);
}
static_assert(f() == 0);
