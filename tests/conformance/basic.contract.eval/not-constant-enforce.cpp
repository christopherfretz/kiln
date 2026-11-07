// [basic.contract.eval]/7.3, /8 (P2900R14): a predicate that is not a core
// constant expression (a call to a non-constexpr function, an exception
// escaping it) is a contract violation in constant evaluation; under
// enforce the evaluation is not a constant expression. kiln's note is the
// reason the predicate is not constant.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=enforce
//@ diag: 22:15 error "constexpr variable 'b1' must be initialized by a constant expression"
//@ diag: 19:34 note "non-constexpr function 'nc' cannot be used in a constant expression"
//@ diag: 22:20 note "in call to 'h(1)'"
//@ diag: 17:5 note "declared here"
//@ diag: 23:15 error "constexpr variable 'b2' must be initialized by a constant expression"
//@ diag: 18:46 note "unhandled exception of type 'int' with content -1 thrown from here"
int nc(int x) { return x; }
constexpr bool thr(int x) { if (x < 0) throw x; return true; }
constexpr int h(const int a) pre(nc(a) > 0) { return a; }
constexpr int k(const int a) pre(thr(a)) { return a; }
static_assert(k(1) == 1);
constexpr int b1 = h(1);
constexpr int b2 = k(-1);
