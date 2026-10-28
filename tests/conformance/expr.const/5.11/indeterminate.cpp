// [expr.const]/5.11: L2R of an object with an indeterminate value.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_11_indeterminate`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 12:10 note "read of uninitialized object is not allowed in a constant expression"
//@ diag: 14:15 note "in call to 'f(false)'"
constexpr int f(bool b) {
  int x;
  if (b)
    x = 1;
  return x;
}
static_assert(f(false) == 0);
