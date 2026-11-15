// GCC 16.1's -fno-contracts at C++26: no function contract specifiers, no
// `contract_assert` statement, no __cpp_contracts. Found running GCC's C++
// testsuite (g++.dg/contracts).
//@ expect: compile-fail
//@ args: -fno-contracts
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 14:13 error "expected ';' after top level declarator"
//@ diag: 15:17 error "use of undeclared identifier 'contract_assert'"
#ifdef __cpp_contracts
#error "__cpp_contracts defined with -fno-contracts"
#endif
int f(int x) pre(x > 0);
void g(int x) { contract_assert(x); }
