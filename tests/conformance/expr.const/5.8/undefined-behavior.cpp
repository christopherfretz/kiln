// [expr.const]/5.8: an operation with undefined behavior (see tests/ub.rs for the catalogue).
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_8_undefined_behavior`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:35 warning "shift count >= width of type"
//@ diag: 10:15 error "static assertion expression is not an integral constant expression"
//@ diag: 9:35 note "shift count 40 >= width of type 'int' (32 bits)"
//@ diag: 10:15 note "in call to 'f(1)'"
constexpr int f(int a) { return a << 40; }
static_assert(f(1) == 0);
