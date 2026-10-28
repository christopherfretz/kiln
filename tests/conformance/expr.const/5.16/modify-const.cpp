// [expr.const]/5.16, [dcl.type.cv]/4: modification of a const object created by the evaluation.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_16_modify_const`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:16 note "modification of object of const-qualified type 'const int' is not allowed in a constant expression"
//@ diag: 14:15 note "in call to 'f()'"
struct S { int a; };
constexpr int f() {
  const S s = {1};
  ((S *)&s)->a = 2;
  return s.a;
}
static_assert(f() == 2);
