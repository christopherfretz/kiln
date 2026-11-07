// [basic.contract.eval]/8 (P2900R14): under observe a contract violation
// during a manifestly constant-evaluated evaluation is diagnosed as a
// warning (GCC's wording, with the call stack) and evaluation continues:
// the result is the one the function computes without the check. A
// predicate that is not a core constant expression (/7.3) is the same
// kind of violation. Each call that violates warns, even at one
// assertion.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=observe
//@ diag: 24:30 warning "contract predicate is false in constant expression"
//@ diag: 27:20 note "in call to 'f(-1)'"
//@ diag: 24:41 warning "contract predicate is false in constant expression"
//@ diag: 29:15 note "in call to 'f(5)'"
//@ diag: 24:59 warning "contract predicate is false in constant expression"
//@ diag: 30:15 note "in call to 'f(9)'"
//@ diag: 24:30 warning "contract predicate is false in constant expression"
//@ diag: 31:15 note "in call to 'f(-2)'"
//@ diag: 25:30 warning "contract condition is not constant"
//@ diag: 32:15 note "in call to 'h(3)'"
int nc(int x) { return x; }
constexpr int f(const int a) pre(a > 0) post(r: r != 6) { contract_assert(a != 9); return a + 1; }
constexpr int h(const int a) pre(nc(a) > 0) { return a; }
constexpr int ok = f(1);
constexpr int b1 = f(-1);
static_assert(b1 == 0);
static_assert(f(5) == 6);
static_assert(f(9) == 10);
static_assert(f(-2) == -1);
static_assert(h(3) == 3);
