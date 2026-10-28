// [expr.const]/5.16: decrement of a bit-field of a global.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_16_decrement_bitfield_global`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:6 note "a constant expression cannot modify an object that is visible outside that expression"
//@ diag: 14:15 note "in call to 'f()'"
struct S { int a : 3; };
S s = {1};
constexpr int f() {
  s.a--;
  return 0;
}
static_assert(f() == 0);
