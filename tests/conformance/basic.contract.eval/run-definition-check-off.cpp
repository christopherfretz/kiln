// -fcontracts-definition-check=off (GCC 16.1's flag; spec/contracts.md
// §6.1): a function no longer checks its own preconditions and
// postconditions, and with the default -fcontracts-client-check=none
// nothing checks them; a contract_assert in its body is still checked.
// Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: assert 'x != 0'
//@ stdout: done 0
#include <contracts>
#include <cstdio>
void handle_contract_violation(const std::contracts::contract_violation &v) {
  static const char *const kinds[] = {"?", "pre", "post", "assert"};
  std::printf("%s '%s'\n", kinds[int(v.kind())], v.comment());
}
int f(int x) pre(x > 0) post(r: r > 0) {
  contract_assert(x != 0);
  return x;
}
int main() { std::printf("done %d\n", f(0)); }
