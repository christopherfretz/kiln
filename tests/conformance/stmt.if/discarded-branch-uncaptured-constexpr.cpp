// [stmt.if]/2: the discarded substatement of a constexpr if statement is
// never executed. Outside a template it may name a local entity a lambda
// does not capture as clang 22 treats it (no odr-use there; GCC 16.1
// rejects it), so the closure has no member for it; evaluating the
// lambda's call runs only the taken branch.
constexpr int f(int a, int b) {
  auto l = [] {
    if constexpr (false) {
      return a + b;
    }
    return 3;
  };
  auto r = [&] {
    if constexpr (true) {
    } else {
      return a;
    }
    return 4;
  };
  static_assert(sizeof(l) == 1);
  static_assert(sizeof(r) == 1);
  return l() + r();
}
static_assert(f(1, 2) == 7);

// The condition is still evaluated once per execution of the statement.
constexpr int g() {
  int n = 0;
  for (int i = 0; i < 3; ++i) {
    if constexpr (sizeof(int) > 1) {
      ++n;
    } else {
      n = -100;
    }
  }
  return n;
}
static_assert(g() == 3);
