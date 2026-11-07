// [stmt.contract.assert] (P2900R14), spec/contracts.md §7: a
// contract_assert dumps as `ContractAssertStmt <loc> assert` with its
// predicate, contextually converted to bool, as the child; a local
// named in it is a const lvalue. clang has no node for it, so the golden
// is kiln's.
//@ expect: ast
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ ast-filter: test
int test(int x, int* p) {
  int n = x;
  contract_assert(n > 0);
  contract_assert(p);
  return n;
}
