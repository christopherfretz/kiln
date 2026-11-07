// [lex.key]/1 (P2900R14): `contract_assert` is a keyword in C++26 at every
// -fclang-compat level (clang 22 reserves it too), so it cannot name a
// variable. GCC 16.1 rejects it too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fclang-compat=22
//@ diag: 10:5 error "expected unqualified-id"
int contract_assert = 0;
