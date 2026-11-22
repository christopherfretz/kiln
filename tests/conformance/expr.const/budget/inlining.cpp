// [implimits]: calls the Oven inlines at their call sites (spec/oven.md
// "Inlining") cost the steps and the depth of a call. The loop calls two
// one-return member functions and a leaf with a loop of its own; 894 is
// the smallest -fconstexpr-steps clang 19 and 22 accept, and kiln with
// and without inlining. The recursion calls an inlined leaf at its
// deepest level: 19 calls of deep and one of val is 20, the
// -fconstexpr-depth limit (inlining-over-limit.cpp: 893 steps and one
// more level fail on both compilers).
//@ args: -fconstexpr-steps=894 -fconstexpr-depth=20
//@ expect: compile-ok
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
static_assert(deep(one, 18) == 19);
