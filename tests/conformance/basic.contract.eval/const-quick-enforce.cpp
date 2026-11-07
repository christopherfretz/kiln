// [basic.contract.eval]/8 (P2900R14): quick_enforce is a terminating
// semantic, so in constant evaluation it behaves as enforce: a violation
// makes the evaluation not a constant expression. (GCC 16.1 accepts the
// same violations in a static_assert under quick_enforce; spec/contracts.md
// §9.)
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=quick_enforce
//@ diag: 19:15 error "constexpr variable 'b1' must be initialized by a constant expression"
//@ diag: 17:30 note "contract predicate is false in constant expression"
//@ diag: 19:20 note "in call to 'f(0)'"
//@ diag: 20:15 error "constexpr variable 'b2' must be initialized by a constant expression"
//@ diag: 17:43 note "contract predicate is false in constant expression"
//@ diag: 20:20 note "in call to 'f(9)'"
constexpr int f(const int a) pre(a > 0) { contract_assert(a != 9); return a + 1; }
static_assert(f(1) == 2);
constexpr int b1 = f(0);
constexpr int b2 = f(9);
