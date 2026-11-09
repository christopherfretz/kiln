// [stmt.expand], [expr.const]/10 (GNU statement expressions): in constant
// evaluation, a statement expression in an expansion statement's header
// that does not jump out (an untaken break/continue, or a loop wholly
// inside it) evaluates normally, in a consteval function, a constexpr
// function, and nested expansion statements; the same functions work at
// run time (header-jump-run.cpp). A taken jump out of a statement
// expression is not a constant expression (header-jump-consteval-neg.cpp).
// GCC 16.1 agrees; clang 22 has no P1306. WP-1081; spec/sema.md §7.6a.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oracle: gcc
struct P { int a, b; };

// Untaken jumps in the init-statement, an element and a destructuring
// initializer, inside an enclosing loop.
consteval int untaken(int n) {
  int s = 0;
  for (int k = 0; k < n; ++k) {
    template for (int i = ({ if (k < 0) continue; k; }); auto x : {1, 2}) { s += x + i; }
    template for (auto x : {1, ({ if (k > n) break; 2; }), 4}) { s += x * 100; }
    template for (auto x : ({ if (k < 0) break; P{k, 1000}; })) { s += x; }
  }
  return s;
}
static_assert(untaken(2) == 3 + 5 + 700 + 700 + 1000 + 1001);

// A loop wholly inside the statement expression keeps its jumps; no
// enclosing loop is needed.
constexpr int inner_loop(int n) {
  int s = 0;
  template for (int i = ({ int j = 0; for (;; ++j) { if (j == n) break; if (j % 2) continue; } j; });
                auto x : {({ int t = 0; while (true) { if (++t > 3) break; } t; }), 10}) {
    s += x * 10 + i;
  }
  return s;
}
static_assert(inner_loop(4) == 40 + 4 + 100 + 4);

// Nested expansion statements: the inner header's untaken jumps target
// the outer one.
consteval int nested(int n) {
  int s = 0;
  template for (auto y : {1, 2, 3}) {
    template for (int i = ({ if (y > n) continue; y; }); auto x : {10, ({ if (y > n) break; 20; })}) {
      s += x + i;
    }
  }
  return s;
}
static_assert(nested(5) == 32 + 34 + 36);

// An untaken return in the header (a taken one leaves the statement
// expression, so it is not a constant expression either).
constexpr int ret(int n) {
  template for (int i = ({ if (n < 0) return -1; n; }); auto x : {1, 2}) { n += x + i; }
  return n;
}
static_assert(ret(1) == 1 + (1 + 1) + (2 + 1));
