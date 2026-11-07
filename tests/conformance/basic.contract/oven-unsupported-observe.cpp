// [basic.contract.eval] (P2900R14): under -fcontract-evaluation-semantic=observe
// a contract assertion reached during constant evaluation is evaluated as
// well (spec/contracts.md §6); until WP-707 kiln reports it unsupported.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=observe
//@ diag: 12:15 error "static assertion expression is not an integral constant expression"
//@ diag: 11:26 note "unsupported: contract evaluation (WP-707)"
//@ diag: 12:15 note "in call to 'g(2)'"
constexpr int g(int x) { contract_assert(x > 0); return x * 2; }
static_assert(g(2) == 4);
