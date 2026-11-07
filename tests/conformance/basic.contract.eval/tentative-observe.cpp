// [basic.contract.eval]/8, [expr.const.init]/4 (P2900R14): a variable's
// initializer is first tried as a constant expression with every contract
// assertion treated as ignored; when that succeeds the variable is
// constant-initialized and the initializer is manifestly constant-evaluated
// again, now checking the assertions, so under observe a violation is a
// warning and the variable is still constant-initialized. An initializer
// that is not a constant expression anyway is a dynamic initialization:
// its assertions are checked at run time and nothing is diagnosed. GCC
// 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ args: -fcontract-evaluation-semantic=observe
//@ diag: 22:30 warning "contract predicate is false in constant expression"
//@ diag: 24:10 note "in call to 'f(-1)'"
//@ diag: 22:43 warning "contract predicate is false in constant expression"
//@ diag: 25:16 note "in call to 'f(9)'"
//@ diag: 22:30 warning "contract predicate is false in constant expression"
//@ diag: 28:27 note "in call to 'f(-3)'"
int nc(int x) { return x; }
constexpr int f(const int a) pre(a > 0) { contract_assert(a != 9); return a + 1; }
int t1 = f(1);
int t2 = f(-1);
const int t3 = f(9);
int d1 = f(nc(-1));
int d2 = nc(1) + f(-1);
void g() { static int s = f(-3); int a = f(-4); (void)a; (void)s; }
