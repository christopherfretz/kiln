// [dcl.contract.func]/7, [basic.contract.eval] (P2900R14): a parameter
// diagnosed by the postcondition const rule stays usable, so the body
// that returns it still evaluates and a constant expression calling the
// function is not disturbed (no "control reached end of constexpr
// function"); a predicate with an error is not evaluated under ignore.
// GCC 16.1 accepts the constant evaluations too.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=ignore
//@ diag: 15:21 error "a value parameter used in a postcondition must be const"
//@ diag: 15:21 note "declared here"
//@ diag: 17:39 error "use of undeclared identifier 'nope'"
constexpr int f(int a) post(r: r == a) { return a; }
static_assert(f(1) == 1);
constexpr int g(const int a) pre(a == nope) { return a; }
static_assert(g(1) == 1);
