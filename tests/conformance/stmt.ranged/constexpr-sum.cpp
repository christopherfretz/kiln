// [stmt.ranged] + [dcl.constexpr]: one constexpr function, evaluated both at
// compile time and at run time, with the same results.
//@ expect: run
//@ std: c++20
//@ args: -O2 -DN=10
//@ stdout-file: constexpr-sum.stdout
#include <array>
#include <cstdio>

constexpr int sum_squares(int n) {
  std::array<int, 16> a{};
  for (int i = 0; i < n; ++i) a[i] = i * i;
  int s = 0;
  for (int v : a) s += v;
  return s;
}

static_assert(sum_squares(N) == 285);

int main() {
  for (int n : {1, 2, N}) std::printf("%d %d\n", n, sum_squares(n));
}
