// -fcontracts-client-check=all with -fcontracts-definition-check=off
// (spec/contracts.md §6.1): a violation handler that throws from a
// caller-side postcondition check of a non-throwing function calls
// std::terminate, as if the exception left the function; for a
// potentially-throwing function the exception reaches the caller.
// Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe -fcontracts-client-check=all -fcontracts-definition-check=off
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: caught
//@ stdout: terminate
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
struct Thrown {};
void handle_contract_violation(const std::contracts::contract_violation &) {
  throw Thrown{};
}
int g(int x) post(r: r > 0) { return x; }
int f(int x) noexcept post(r: r > 0) { return x; }
int main() {
  std::set_terminate([] {
    std::puts("terminate");
    std::exit(0);
  });
  try {
    g(-1);
  } catch (Thrown) {
    std::puts("caught");
  }
  try {
    f(-1);
  } catch (Thrown) {
    std::puts("not terminated");
  }
  return 1;
}
