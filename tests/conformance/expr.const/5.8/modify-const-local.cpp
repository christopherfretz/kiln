// [dcl.type.cv]/4: modifying a const object.
// Mirrors crates/kiln_oven/tests/ub.rs `ub_modify_const_local`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:15 error "static assertion expression is not an integral constant expression"
//@ diag: 10:14 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 13:15 note "in call to 'f()'"
constexpr int f() {
  const int c = 1;
  *(int *)&c = 2;
  return c;
}
static_assert(f() == 2);
