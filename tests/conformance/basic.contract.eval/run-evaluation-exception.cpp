// [basic.contract.eval]/7 (P2900R14): an exception escaping the predicate
// is a contract violation with detection mode evaluation_exception, and
// the handler is invoked from within an implicit handler for it, so
// std::current_exception() is that exception; under observe execution
// then continues. A handler that throws propagates its exception out of
// the checked function. GCC 16.1 reports predicate_false for the escaping
// exception, though inside the handler (spec/contracts.md §9), so there is
// no oracle.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=observe
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ stdout: handler 3 2 1
//@ stdout: next 4
//@ stdout: handler 1 1 0
//@ stdout: caught 42
#include <contracts>
#include <cstdio>
#include <exception>
bool check(int x) {
  if (x == 3) throw 1;
  return x != 0;
}
bool rethrow = false;
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("handler %d %d %d\n", int(v.kind()), int(v.mode()),
              std::current_exception() != nullptr);
  if (rethrow) throw 42;
}
int g(int y) { contract_assert(check(y)); return y + 1; }
int f(int x) pre(check(x)) { return x; }
int main() {
  std::printf("next %d\n", g(3));
  rethrow = true;
  try {
    f(0);
    std::printf("not reached\n");
  } catch (int e) {
    std::printf("caught %d\n", e);
  }
}
