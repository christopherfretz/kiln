// [stmt.contract.assert]/1 (P2900R14): the predicate is parenthesized and
// the statement ends with a semicolon. GCC 16.1 rejects both too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 9:34 error "expected '(' after 'contract_assert'"
//@ diag: 10:36 error "expected ';' after contract_assert statement"
void s1(int x) { contract_assert x; }
void s2(int x) { contract_assert(x) }
