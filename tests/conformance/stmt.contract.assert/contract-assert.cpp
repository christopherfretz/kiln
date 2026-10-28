// [stmt.contract.assert]: `contract_assert` is a statement (P2900, C++26).
// A skip rather than an xfail: clang 19's parse errors say nothing useful.
//@ expect: run
//@ skip(clang): clang 19 does not implement contracts (P2900)
int main() {
  int x = 2;
  contract_assert(x + x == 4);
  return x - 2;
}
