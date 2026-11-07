// [basic.contract.eval]/7.3, /8 (P2900R14): under observe a predicate that
// is not a core constant expression (a call to a non-constexpr function,
// an exception escaping it) is a violation diagnosed as a warning, and
// constant evaluation continues. GCC 16.1 rejects the escaping exception
// (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ args: -fcontract-evaluation-semantic=observe
//@ diag: 16:30 warning "contract condition is not constant"
//@ diag: 18:15 note "in call to 'h(1)'"
//@ diag: 17:30 warning "contract condition is not constant"
//@ diag: 19:15 note "in call to 'k(-1)'"
int nc(int x) { return x; }
constexpr bool thr(int x) { if (x < 0) throw x; return true; }
constexpr int h(const int a) pre(nc(a) > 0) { return a; }
constexpr int k(const int a) pre(thr(a)) { return a; }
static_assert(h(1) == 1);
static_assert(k(-1) == -1);
