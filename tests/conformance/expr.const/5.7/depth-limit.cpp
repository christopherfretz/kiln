// [expr.const]/5.7, [implimits]: recursion past the implementation's depth limit (512, as clang).
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_7_depth_limit`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:46 note "constexpr evaluation exceeded maximum depth of 512 calls"
//@ diag: 18:46 note "in call to 'f(489)'"
//@ diag: 18:46 note "in call to 'f(490)'"
//@ diag: 18:46 note "in call to 'f(491)'"
//@ diag: 18:46 note "in call to 'f(492)'"
//@ diag: 18:46 note "in call to 'f(493)'"
//@ diag: 18:46 note "(skipping 502 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 18:46 note "in call to 'f(996)'"
//@ diag: 18:46 note "in call to 'f(997)'"
//@ diag: 18:46 note "in call to 'f(998)'"
//@ diag: 18:46 note "in call to 'f(999)'"
//@ diag: 19:15 note "in call to 'f(1000)'"
constexpr int f(int n) { return n == 0 ? 0 : f(n - 1); }
static_assert(f(1000) == 0);
