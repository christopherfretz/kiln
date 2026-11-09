// [expr.const]/16, [expr.const]/5.1, [implimits]: an immediate invocation
// is evaluated once (WP-1129). 105 is the smallest -fconstexpr-steps
// clang 19 accepts for sum(100) (immediate-invocation-once-over-limit.cpp
// checks 104 fails on both compilers), so each invocation below passes
// with N just above one evaluation's cost; the budgets state that each is
// one reported evaluation of that cost, including those whose
// completion checks (overflow, conversion) fold them, and one per use of
// a default argument (two uses).
//@ args: -fconstexpr-steps=105
//@ oven: 22 evaluations<=1 steps<=105
//@ oven: 23 evaluations<=1 steps<=105
//@ oven: 24 evaluations<=1 steps<=105
//@ oven: 25 evaluations<=1 steps<=105
//@ oven: 26 evaluations<=2 steps<=210
//@ oven: evaluations<=6 steps<=630
consteval int sum(int n) {
  int r = 0;
  for (int i = 0; i < n; ++i)
    r += i;
  return r;
}
int ovf(int x) { return x + sum(100); }
short conv() { return sum(100); }
int cond() { return sum(100) ? 1 : 2; }
bool cmp(int x) { return x < sum(100); }
int def(int x = sum(100)) { return x; }
int uses() { return def() + def(); }
