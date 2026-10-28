// [expr.const]/5.7, [implimits]: an evaluation past the step limit (1048576, as clang).
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_7_step_limit`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:5 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 14:15 note "in call to 'f()'"
constexpr int f() {
  int i = 0;
  while (true)
    ++i;
  return i;
}
static_assert(f() == 0);
