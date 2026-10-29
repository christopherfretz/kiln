// [stmt.pre]/4, [stmt.for]: placeholders in conditions, for-init
// statements and block scope deduce like any variable.
//@ strict-diags
constexpr int f(int n) {
  int s = 0;
  for (auto i = 0; i < n; ++i)
    s += i;
  if (auto t = s * 2)
    return t;
  while (auto z = false)
    return -1;
  return 0;
}
static_assert(f(4) == 12 && f(0) == 0);
constexpr auto g() {
  static_assert(__is_same(decltype(f(1)), int));
  auto a = 1u, b = 2u;
  return a + b;
}
static_assert(__is_same(decltype(g()), unsigned) && g() == 3);
