// [implimits]: one step fewer than inlining.cpp needs crosses the limit,
// and one more level of recursion crosses the depth limit at the inlined
// call of val (spec/oven.md "Inlining": an inlined call counts as a call).
//@ args: -fconstexpr-steps=893 -fconstexpr-depth=20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 44:15 error "static assertion expression is not an integral constant expression"
//@ diag: 42:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 44:15 note "in call to 'total(16)'"
//@ diag: 50:15 error "static assertion expression is not an integral constant expression"
//@ diag: 47:10 note "constexpr evaluation exceeded maximum depth of 20 calls"
//@ diag: 47:33 note "in call to 'deep(one, 0)'"
//@ diag: 47:33 note "in call to 'deep(one, 1)'"
//@ diag: 47:33 note "in call to 'deep(one, 2)'"
//@ diag: 47:33 note "in call to 'deep(one, 3)'"
//@ diag: 47:33 note "in call to 'deep(one, 4)'"
//@ diag: 47:33 note "(skipping 10 calls in backtrace; use -fconstexpr-backtrace-limit=0 to see all)"
//@ diag: 47:33 note "in call to 'deep(one, 15)'"
//@ diag: 47:33 note "in call to 'deep(one, 16)'"
//@ diag: 47:33 note "in call to 'deep(one, 17)'"
//@ diag: 47:33 note "in call to 'deep(one, 18)'"
//@ diag: 50:15 note "in call to 'deep(one, 19)'"
struct Arr {
  int v[8];
  constexpr int get(int i) const { return v[i]; }
  constexpr int size() const { return 8; }
};
constexpr int count(const int *p) {
  int n = 0;
  for (int i = 0; i < *p; ++i)
    n += 1;
  return n;
}
constexpr int total(int rounds) {
  Arr a{{1, 2, 3, 4, 5, 6, 7, 8}};
  int s = 0;
  for (int r = 0; r < rounds; ++r) {
    for (int i = 0; i < a.size(); ++i)
      s += a.get(i);
    s += count(&a.v[r % 8]);
  }
  return s;
}
static_assert(total(16) == 648);
constexpr int val(const int &r) { return r; }
constexpr int deep(const int &r, int n) {
  return val(r) + (n == 0 ? 0 : deep(r, n - 1));
}
constexpr int one = 1;
static_assert(deep(one, 19) == 20);
