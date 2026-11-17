// run-client-check-declared-template.cpp under the ignore evaluation
// semantic ([basic.contract.eval]): the caller-side precondition of a
// function template specialization and a member function template
// specialization defined in another translation unit is not evaluated.
// GCC 16.1 checks the structured binding's `get` and fails with an
// internal compiler error on the others, so the output is kiln's own
// (spec/codegen.md, "Caller-side contract checks"). Found running
// GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=ignore -fcontracts-client-check=pre -fcontracts-definition-check=off
//@ unit: Inputs/declared-template-contracts-defs.cpp
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: 2 1
//@ stdout: 1 1
//@ stdout: done
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
#include "Inputs/declared-template-contracts.h"
void handle_contract_violation(const std::contracts::contract_violation &v) {
  std::printf("handler '%s'\n", v.comment());
}
int main() {
  std::set_terminate([] {
    std::printf("terminate\n");
    std::fflush(stdout);
    std::_Exit(7);
  });
  S s;
  std::printf("%d %d\n", f(6, 0L), s.m(1, 0));
  std::printf("%d %d\n", f(0, 0), s.m(0, 0));
  std::puts("done");
}
