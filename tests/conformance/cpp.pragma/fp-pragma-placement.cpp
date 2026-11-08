// [cpp.pragma]/1, C23 7.12.2, [cfenv.syn]: the floating-point pragmas
// appear at file scope or at the start of a compound statement (before
// any declaration or statement); elsewhere clang rejects them.
// `float_control` push/pop is namespace-scope only; a pop with an empty
// stack is a warning.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 19:14 error "this pragma cannot appear in struct declaration"
//@ diag: 21:15 error "this pragma cannot appear in struct declaration"
//@ diag: 23:9 error "this pragma cannot appear in struct declaration"
//@ diag: 29:14 error "'#pragma fp_contract' can only appear at file scope or at th"
//@ diag: 31:15 error "'#pragma clang fp' can only appear at file scope or at the s"
//@ diag: 33:9 error "'#pragma float_control' can only appear at file scope or at"
//@ diag: 38:9 error "'#pragma float_control push/pop' can only appear at file or"
//@ diag: 39:9 error "'#pragma float_control push/pop' can only appear at file or"
//@ diag: 48:9 warning "#pragma float_control(pop, ...) failed: stack empty"
struct S {
#pragma STDC FP_CONTRACT ON
  int a;
#pragma clang fp contract(fast)
  int b;
#pragma float_control(precise, on)
  int c;
};

double f(double a, double b, double c) {
  double r = a * b;
#pragma STDC FP_CONTRACT OFF
  r += c;
#pragma clang fp reassociate(on)
  r *= c;
#pragma float_control(precise, off)
  return r;
}

void g() {
#pragma float_control(push)
#pragma float_control(pop)
}

namespace n {
#pragma float_control(push)
#pragma float_control(precise, on)
#pragma float_control(pop)
}

#pragma float_control(pop)

double h(double a, double b, double c) {
#pragma STDC FP_CONTRACT OFF
#pragma clang fp contract(fast) reassociate(off) reciprocal(on)
#pragma float_control(precise, on)
  return a * b + c;
}
