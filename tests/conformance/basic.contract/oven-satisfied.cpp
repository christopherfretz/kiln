// [basic.contract.eval] (P2900R14): a contract assertion reached during
// constant evaluation under a checking semantic (here the default,
// enforce) is evaluated; satisfied assertions leave the evaluation a
// constant expression, and a function with contracts that is not
// constant-evaluated is unaffected. GCC 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
constexpr int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }
constexpr int g(int x) { contract_assert(x > 0); return x * 2; }
constexpr int h(int x) { return x; }
static_assert(f(1) == 2);
static_assert(g(2) == 4);
static_assert(h(3) == 3);
int run(int x) { return f(x) + g(x); }
