// -fcontracts-definition-check=off in a constant evaluation (GCC
// 16.1's flag; spec/contracts.md §6.4): a kind of assertion that no
// side checks at run time is not checked in a constant evaluation
// either, so the violated precondition is not diagnosed. Found running
// GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-ok
//@ args: -fcontracts-definition-check=off
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
constexpr int f(int x) pre(x > 0) post(r: r > 0) { return x; }
static_assert(f(0) == 0);
