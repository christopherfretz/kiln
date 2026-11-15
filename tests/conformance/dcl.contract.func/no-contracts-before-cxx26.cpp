// [dcl.contract.func] is C++26: without -fcontracts, `pre` and `post` are
// ordinary names before it, and __cpp_contracts is not defined. Found
// running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-ok
//@ std: c++23
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
#ifdef __cpp_contracts
#error "__cpp_contracts defined before C++26"
#endif
int pre(int x) { return x; }
int post = pre(1);
