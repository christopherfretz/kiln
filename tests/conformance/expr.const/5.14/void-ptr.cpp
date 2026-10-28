// [expr.const]/5.14: a conversion from `cv void*` to a pointer to a type not similar to the object's.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_14_void_ptr`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:11 note "cast from 'void *' is not allowed in a constant expression because the pointed object type 'int' is not similar to the target type 'char'"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  int x = 1;
  void *v = &x;
  return *static_cast<char *>(v);
}
static_assert(f() == 1);
