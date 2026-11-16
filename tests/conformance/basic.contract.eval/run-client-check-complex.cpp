// -fcontracts-client-check=all with -fcontracts-definition-check=off
// (spec/contracts.md §6.1) on a call with `_Complex` arguments and a
// `_Complex` result: the caller checks the preconditions on the values
// the callee gets (each argument evaluated once) and the postcondition
// on the returned value, however the target passes them (the handler
// prints only the kinds: GCC's comments drop `__real__` and `__imag__`). Found running
// GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=all -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: pre pre f
//@ stdout: post -1 0 1
//@ stdout: f
//@ stdout: 2 1 1
//@ stdout: done
#include <contracts>
#include <cstdio>
void handle_contract_violation(const std::contracts::contract_violation &v) {
  static const char *const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s ", kinds[int(v.kind())]);
}
int calls;
_Complex double f(_Complex double z, _Complex float w)
    pre(__real__ z > 0) pre(__imag__ w > 0) post(r: __imag__ r > 0) {
  std::puts("f");
  _Complex double r = z;
  __imag__ r = __imag__ w;
  return r;
}
_Complex double arg(double re, double im) {
  ++calls;
  _Complex double z = re;
  __imag__ z = im;
  return z;
}
int main() {
  _Complex float w = 0;
  _Complex double r = f(arg(-1, 0), w);
  std::printf("%g %g %d\n", __real__ r, __imag__ r, calls);
  __imag__ w = 1;
  r = f(arg(2, 0), w);
  std::printf("%g %g %d\n", __real__ r, __imag__ r, calls - 1);
  std::puts("done");
}
