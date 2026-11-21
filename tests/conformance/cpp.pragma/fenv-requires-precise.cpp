// Microsoft's rule clang enforces: `#pragma STDC FENV_ACCESS ON` and
// `#pragma float_control(except, on)` need precise FP semantics (no
// value-changing pragma or flag in effect), and `float_control(precise,
// off)` needs exceptions ignored and FENV_ACCESS off (clang
// `err_pragma_fenv_requires_precise`, `err_pragma_fc_*`; WP-1744).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 17:9 error "'#pragma float_control(except, on)' is illegal when precise is disabled"
//@ diag: 19:14 error "'#pragma STDC FENV_ACCESS ON' is illegal when precise is disabled"
//@ diag: 23:9 error "'#pragma float_control(precise, off)' is illegal when except is enabled"
//@ diag: 27:9 error "'#pragma float_control(precise, off)' is illegal when fenv_access is enabled"
//@ diag: 32:9 error "'#pragma float_control(precise, off)' is illegal when except is enabled"
//@ diag: 41:14 error "'#pragma STDC FENV_ACCESS ON' is illegal when precise is disabled"
//@ diag: 46:9 error "'#pragma float_control(except, on)' is illegal when precise is disabled"
#pragma float_control(precise, off)
#pragma float_control(except, on)
#pragma clang fp reassociate(on)
#pragma STDC FENV_ACCESS ON
#pragma float_control(precise, on)
#pragma STDC FENV_ACCESS ON
void f() {
#pragma float_control(precise, off)
}
void g() {
#pragma clang fp exceptions(maytrap)
#pragma float_control(precise, off)
}
#pragma STDC FENV_ACCESS OFF
void h() {
#pragma clang fp exceptions(strict)
#pragma float_control(precise, off)
}
void k() {
#pragma float_control(except, on)
#pragma clang fp exceptions(ignore)
#pragma float_control(precise, off)
}
void m() {
#pragma clang fp reciprocal(on)
#pragma STDC FENV_ACCESS ON
}
void n() {
#pragma clang fp exceptions(maytrap)
#pragma float_control(precise, off)
#pragma float_control(except, on)
}
