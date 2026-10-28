// [expr.const]/5.14: a cast from void* to a dissimilar object type.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_pun_void_cast`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:11 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'double' is not similar to the target type 'long'"
//@ diag: 13:15 note "in call to 'f()'"
constexpr long f() {
  double d = 1.0;
  void *v = &d;
  return *static_cast<long *>(v);
}
static_assert(f() == 0);
