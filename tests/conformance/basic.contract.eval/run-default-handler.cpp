// [basic.contract.handler]/2-4 (P2900R14): without a user-provided
// handle_contract_violation the default handler runs. kiln provides
// <contracts> and a replaceable default handler when the selected
// standard library has none (spec/contracts.md §6.3); the default prints
// GCC's message to stderr and returns, so under observe the program
// continues. GCC links its default from -lstdc++exp, so there is
// no oracle.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: 2 1
#include <contracts>
#include <cstdio>
int f(int x) pre(x > 0) post(r: r > 1) { return x + 1; }
int main() {
  int a = f(1);
  int b = f(0);
  std::printf("%d %d\n", a, b);
}
