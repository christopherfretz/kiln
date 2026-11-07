// [basic.contract.eval]/10 (P2900R14): under quick_enforce a violation
// terminates the program without invoking the contract-violation handler;
// a replaced handler is never called.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=quick_enforce
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ exit: 5
//@ stdout: before
//@ stdout: terminate
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
void handle_contract_violation(const std::contracts::contract_violation&) {
  std::printf("handler\n");
}
int main() {
  std::set_terminate([] {
    std::printf("terminate\n");
    std::fflush(stdout);
    std::_Exit(5);
  });
  int a = 1;
  contract_assert(a == 1);
  std::printf("before\n");
  contract_assert(a == 2);
  std::printf("after\n");
}
