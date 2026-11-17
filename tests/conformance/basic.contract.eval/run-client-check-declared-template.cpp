// -fcontracts-client-check=all (spec/contracts.md §6.1) on calls of
// function template specializations whose definition is not in the
// calling translation unit: a member function template, a function
// template (implicit and explicit specialization: the explicit one has
// only its own assertions, [temp.expl.spec]/12), a member of a class
// template, a specialization under an explicit instantiation
// declaration, and the `get` a structured binding calls
// ([dcl.struct.bind]/4). The caller instantiates their function
// contract assertions from the template's declaration and checks
// them. The definitions check nothing (-fcontracts-definition-check=off).
// GCC 16.1 checks the structured binding's `get` and fails with an
// internal compiler error on the others, so the output is kiln's own
// (spec/codegen.md, "Caller-side contract checks"). Found running
// GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=all -fcontracts-definition-check=off
//@ unit: Inputs/declared-template-contracts-defs.cpp
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: pre 'x > 0'
//@ stdout: pre 'x > 0'
//@ stdout: pre 'x > 5'
//@ stdout: pre 'x > 0'
//@ stdout: pre 'x > 0'
//@ stdout: pre 'v > 0'
//@ stdout: pre 'v > 0'
//@ stdout: 1 1 2 1 1 0 1
//@ stdout: post 'r > x'
//@ stdout: post 'r > x'
//@ stdout: post 'r > x'
//@ stdout: 1 1 2 1 1 0 1
#include <contracts>
#include <cstdio>
#include "Inputs/declared-template-contracts.h"
void handle_contract_violation(const std::contracts::contract_violation &v) {
  static const char *const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s '%s'\n", kinds[int(v.kind())], v.comment());
}
int run(int x) {
  S s;
  C<int> c;
  int a = s.m(x, 0);
  int b = f(x, 0);
  int d = f(x, 0L);
  int g = c.m(x, 0);
  int h = e(x, 0);
  auto [p0, p1] = P{x};
  std::printf("%d %d %d %d %d %d %d\n", a, b, d, g, h, p0, p1);
  return 0;
}
int main() {
  run(0);
  run(9);
}
