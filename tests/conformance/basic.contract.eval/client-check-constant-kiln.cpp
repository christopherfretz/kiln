// -fcontracts-client-check= with -fcontracts-definition-check=off in a
// constant evaluation (spec/contracts.md §6.4): a kind of assertion a
// caller checks at run time is checked in a constant evaluation too, so
// under `pre` the postcondition is not checked (`f(1)` is constant) and
// under `all` it is. No oracle: GCC 16.1 rejects the second as "call to
// non-constexpr function" (its caller-side wrapper is not constexpr).
// Found running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-fail
//@ args: -fcontracts-client-check=all -fcontracts-definition-check=off
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 16:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:35 note "contract predicate is false in constant expression"
//@ diag: 16:15 note "in call to 'f(1)'"
constexpr int f(int x) pre(x > 0) post(r: r > 1) { return x; }
static_assert(f(1) == 1);
