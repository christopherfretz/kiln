// [stmt.expand]/5.2 (P1306R5): an iterating expansion over 10000 elements
// whose iterator is a user class, so N is counted by evaluating
// `begin + i != end` per element rather than by `end - begin` (the pointer
// and standard library fast path). The count and the elements share one
// `static constexpr auto __iter = begin + i;` per element (spec/sema.md
// §7.6a, WP-467): line 26's evaluations are N+3 `__begin`/`__end`/`__iter`
// initializations, N+1 count tests and N element initializations, each a
// few steps; a count that re-evaluates `begin + i` doubles its steps.
// docs/status/perf-history.md "WP-467" records wall time and peak memory.
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=23
//@ oven: 26 evaluations<=30005 steps<=61000
struct It {
  const int *p;
  constexpr It operator+(long n) const { return {p + n}; }
  constexpr bool operator!=(It o) const { return p != o.p; }
  constexpr const int &operator*() const { return *p; }
};
struct R {
  int a[10000];
  constexpr It begin() const { return {a}; }
  constexpr It end() const { return {a + 10000}; }
};
constexpr R big = [] { R r{}; for (int i = 0; i < 10000; ++i) r.a[i] = i % 7; return r; }();
constexpr long sum() { long s = 0; template for (constexpr int x : big) { s += x; } return s; }
static_assert(sum() == 29994);
