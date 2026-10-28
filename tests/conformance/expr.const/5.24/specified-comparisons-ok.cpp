// [expr.const]/5.24, [expr.eq]/3, [expr.rel]/4: comparisons with specified results.
// Mirrors crates/kiln_oven/tests/expr_const.rs `expr_const_5_24_specified_comparisons_ok`.
constexpr int x = 1, y = 2;
constexpr int a[3] = {1, 2, 3};
struct S { int m; int n; };
constexpr S s = {1, 2};
static_assert(&x != &y);
static_assert(&x != nullptr);
static_assert(a < a + 2 && a + 3 > a + 1);
static_assert(&s.m < &s.n);
static_assert(a + 3 == a + 3);
