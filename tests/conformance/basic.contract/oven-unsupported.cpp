// [basic.contract.eval] (P2900R14): a contract assertion reached during
// constant evaluation under a checking semantic (here the default,
// enforce) is evaluated; until WP-707 implements that, kiln reports the
// evaluation unsupported rather than skipping the check
// (spec/contracts.md §6). A function with contracts that is not
// constant-evaluated is unaffected. GCC 16.1 evaluates the (true)
// predicates, so there is no oracle.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:30 note "unsupported: contract evaluation (WP-707)"
//@ diag: 20:15 note "in call to 'f(1)'"
//@ diag: 21:15 error "static assertion expression is not an integral constant expression"
//@ diag: 18:26 note "unsupported: contract evaluation (WP-707)"
//@ diag: 21:15 note "in call to 'g(2)'"
constexpr int f(const int x) pre(x > 0) post(r: r > x) { return x + 1; }
constexpr int g(int x) { contract_assert(x > 0); return x * 2; }
constexpr int h(int x) { return x; }
static_assert(f(1) == 2);
static_assert(g(2) == 4);
static_assert(h(3) == 3);
