// [stmt.expand], [expr.const]/10 (GNU statement expressions): a taken
// break, continue or return out of a statement expression in an expansion
// statement's header is not a constant expression, wherever it jumps to.
// clang rejects every jump out of a statement expression in constant
// evaluation ("this use of statement expressions is not supported", at the
// jump), and its P1306 implementation keeps that rule for expansion
// statements; kiln follows it. GCC 16.1 evaluates these jumps (an
// extension; header-jump-run.cpp has their run-time results).
// header-jump-consteval.cpp has the untaken ones. WP-1081; spec/sema.md
// §7.6a.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=23
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:30 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 38:15 note "in call to 'init_continue(3)'"
//@ diag: 47:15 error "static assertion expression is not an integral constant expression"
//@ diag: 43:35 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 47:15 note "in call to 'element_break(3)'"
//@ diag: 56:15 error "static assertion expression is not an integral constant expression"
//@ diag: 52:31 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 56:15 note "in call to 'destructuring_continue(3)'"
//@ diag: 65:15 error "static assertion expression is not an integral constant expression"
//@ diag: 61:30 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 65:15 note "in call to 'nested(2)'"
//@ diag: 72:15 error "static assertion expression is not an integral constant expression"
//@ diag: 69:33 note "this use of statement expressions is not supported in a constant expression"
//@ diag: 72:15 note "in call to 'ret(2)'"
struct P { int a, b; };

constexpr int init_continue(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (int i = ({ if (k == 1) continue; 0; }); auto x : {1, 2}) { s += x + i; }
  }
  return s;
}
static_assert(init_continue(3) == 6);

constexpr int element_break(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (auto x : {1, ({ if (k == 1) break; 2; }), 4}) { s += x; }
  }
  return s;
}
static_assert(element_break(3) == 15);

constexpr int destructuring_continue(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (auto x : ({ if (k == 1) continue; P{1, 2}; })) { s += x; }
  }
  return s;
}
static_assert(destructuring_continue(3) == 6);

consteval int nested(int n) {
  int s = 0;
  template for (auto y : {1, 2, 3}) {
    template for (int i = ({ if (y == n) continue; 0; }); auto x : {10, 20}) { s += x + i; }
  }
  return s;
}
static_assert(nested(2) == 60);

constexpr int ret(int n) {
  int s = 0;
  template for (auto x : {1, ({ if (n == 2) return -1; 2; })}) { s += x; }
  return s;
}
static_assert(ret(2) == -1);
