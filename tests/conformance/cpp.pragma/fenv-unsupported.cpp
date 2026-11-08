// [cfenv.syn], C23 7.6.1, 7.6.2: kiln's floating-point environment is the
// default one (round to nearest, no status-flag access, no traps,
// FLT_EVAL_METHOD 0). The pragmas that would change it are errors, never
// ignored; the ones that keep the default are accepted. FENV_ROUND also
// has clang's "not supported" warning (WP-806).
//@ expect: compile-fail
//@ skip(clang): kiln rejects the floating-point environment pragmas clang implements
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 23:14 error "'#pragma STDC FENV_ACCESS ON' (the floating-point environment) is not supported yet"
//@ diag: 24:34 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 24:34 error "'#pragma STDC FENV_ROUND FE_UPWARD' (the floating-point environment) is not supported yet"
//@ diag: 25:9 error "'#pragma float_control(except, on)' (the floating-point environment) is not supported yet"
//@ diag: 26:15 error "'#pragma clang fp exceptions(strict)' (the floating-point environment) is not supported yet"
//@ diag: 27:15 error "'#pragma clang fp exceptions(maytrap)' (the floating-point environment) is not supported yet"
//@ diag: 28:15 error "'#pragma clang fp eval_method(double)' (the floating-point environment) is not supported yet"
//@ diag: 30:37 warning "pragma STDC FENV_ROUND is not supported"
//@ diag: 35:14 error "'#pragma STDC FENV_ACCESS ON' (the floating-point environment) is not supported yet"
//@ diag: 40:15 error "'#pragma clang fp eval_method(extended)' (the floating-point environment) is not supported yet"
// The pragmas are reported where they take effect: at file scope and at
// the start of a compound statement.

#pragma STDC FENV_ACCESS ON
#pragma STDC FENV_ROUND FE_UPWARD
#pragma float_control(except, on)
#pragma clang fp exceptions(strict)
#pragma clang fp exceptions(maytrap)
#pragma clang fp eval_method(double)
#pragma STDC FENV_ACCESS OFF
#pragma STDC FENV_ROUND FE_TONEAREST
#pragma float_control(except, off)
#pragma clang fp exceptions(ignore)
#pragma clang fp eval_method(source)
double f(double a, double b) {
#pragma STDC FENV_ACCESS ON
  return a + b;
}

double g(double a, double b) {
#pragma clang fp eval_method(extended)
  return a * b;
}
