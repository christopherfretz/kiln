// [basic.contract.handler]/4, [support.contract.violation] (P2900R14): a
// replacing handler may call
// std::contracts::invoke_default_contract_violation_handler, which kiln
// defines (replaceably) when the standard library has no <contracts>
// (spec/contracts.md §6.3); it prints to stderr and returns. GCC links its
// default from -lstdc++exp, so there is no oracle.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: mine
//@ stdout: back 1
#include <contracts>
#include <cstdio>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("mine\n");
  std::fflush(stdout);
  std::contracts::invoke_default_contract_violation_handler(v);
}
int main() {
  int x = 1;
  contract_assert(x == 2);
  std::printf("back %d\n", x);
}
