// [implimits]: the step limit reached inside the body of an inlined call
// (spec/oven.md "Inlining") is diagnosed at the statement in the callee,
// with the "in call to" note of the inlined call, as clang 19 and 22 do.
//@ args: -fconstexpr-steps=881
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 21:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 32:10 note "in call to 'count(&a.v[7])'"
//@ diag: 36:15 note "in call to 'total(16)'"
//@ diag: 45:15 error "static assertion expression is not an integral constant expression"
//@ diag: 17:34 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 42:12 note "in call to 'a.get(5)'"
//@ diag: 45:15 note "in call to 'gets(100)'"
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
constexpr int gets(int rounds) {
  Arr a{{1, 2, 3, 4, 5, 6, 7, 8}};
  int s = 0;
  for (int r = 0; r < rounds; ++r)
    for (int i = 0; i < 8; ++i)
      s += a.get(i);
  return s;
}
static_assert(gets(100) == 3600);
