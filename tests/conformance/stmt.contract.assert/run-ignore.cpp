// [stmt.contract.assert], [basic.contract.eval]/1 (P2900R14): under the
// ignore semantic a contract_assert is not evaluated: neither its
// predicate's side effects nor a violation happen. GCC 16.1 prints the
// same.
//@ expect: run
//@ args: -fcontract-evaluation-semantic=ignore
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ stdout: 0 1
extern "C" int printf(const char*, ...);
int calls;
bool side() { ++calls; return false; }
int main() {
  contract_assert(side());
  contract_assert(false);
  int x = 1;
  printf("%d %d\n", calls, x);
}
