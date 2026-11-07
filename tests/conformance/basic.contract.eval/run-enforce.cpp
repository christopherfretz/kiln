// [basic.contract.eval]/10 (P2900R14): under enforce a violation invokes
// the contract-violation handler and then terminates the program
// (std::terminate, via the terminate handler, which here exits with a
// status of its own); is_terminating() says so. Assertions that hold do
// nothing, and the first violated one terminates before the body runs.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=enforce
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ exit: 7
//@ stdout: ok 3
//@ stdout: handler 1 3 1 1 'x != 2'
//@ stdout: terminate
#include <contracts>
#include <cstdio>
#include <cstdlib>
#include <exception>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("handler %d %d %d %d '%s'\n", int(v.kind()), int(v.semantic()), int(v.mode()),
              int(v.is_terminating()), v.comment());
}
int f(const int x) pre(x != 2) post(r: r == x + 1) {
  std::printf("body %d\n", x);
  return x + 1;
}
int main() {
  std::set_terminate([] {
    std::printf("terminate\n");
    std::fflush(stdout);
    std::_Exit(7);
  });
  int a = 2;
  contract_assert(a == 2);
  std::printf("ok %d\n", a + 1);
  std::fflush(stdout);
  return f(a);
}
