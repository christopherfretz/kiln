export module lcm;
export inline int lcm_sum(int x) {
  struct P { int a, b; int sum() const { return a + b; } };
  return P{x, 2}.sum();
}
