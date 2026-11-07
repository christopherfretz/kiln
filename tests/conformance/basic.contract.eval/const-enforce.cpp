// [basic.contract.eval]/8, [expr.const]/10 (P2900R14): under enforce a
// contract violation during a manifestly constant-evaluated evaluation
// makes it not a constant expression: a false precondition, postcondition
// or contract_assert is diagnosed with GCC's wording and the call stack.
// The postcondition reads the result in place (a class result, a
// reference result); satisfied assertions change nothing.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=enforce
//@ diag: 38:15 error "constexpr variable 'b1' must be initialized by a constant expression"
//@ diag: 27:30 note "contract predicate is false in constant expression"
//@ diag: 38:20 note "in call to 'f(-1)'"
//@ diag: 39:15 error "constexpr variable 'b2' must be initialized by a constant expression"
//@ diag: 27:41 note "contract predicate is false in constant expression"
//@ diag: 39:20 note "in call to 'f(5)'"
//@ diag: 40:15 error "constexpr variable 'b3' must be initialized by a constant expression"
//@ diag: 27:59 note "contract predicate is false in constant expression"
//@ diag: 40:20 note "in call to 'f(9)'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:28 note "contract predicate is false in constant expression"
//@ diag: 41:15 note "in call to 'g(3)'"
//@ diag: 42:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:70 note "contract predicate is false in constant expression"
//@ diag: 42:15 note "in call to 'loop(5)'"
constexpr int f(const int a) pre(a > 0) post(r: r != 6) { contract_assert(a != 9); return a + 1; }
struct S { int x; };
constexpr S g(const int a) post(r: r.x == 2 * a) { S s{a}; if (a == 3) s.x = 1; return s; }
constexpr const int& id(const int& a) post(r: &r == &a) { return a; }
constexpr int loop(int n) { int s = 0; for (int i = 0; i < n; ++i) { contract_assert(i < 4); s += i; } return s; }
constexpr int ok = f(1);
static_assert(f(2) == 3);
static_assert(g(0).x == 0);
constexpr int gi = 4;
static_assert(id(gi) == 4);
static_assert(loop(4) == 6);
constexpr int b1 = f(-1);
constexpr int b2 = f(5);
constexpr int b3 = f(9);
static_assert(g(3).x == 1);
static_assert(loop(5) == 10);
