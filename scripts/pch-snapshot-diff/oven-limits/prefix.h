// Limits and notes into prefix code (WP-1396): the main file's failing
// evaluations run code compiled in the prefix, so the step limit, the
// call-stack notes and their locations come from the restored Oven.
#include <array>
#include <cstddef>
namespace pfx {
constexpr long spin(long n) {
  long s = 0;
  for (long i = 0; i < n; ++i) s += i % 7;
  return s;
}
inline constexpr long small_spin = spin(1000);
constexpr int deep(int n) { return n == 0 ? 0 : 1 + deep(n - 1); }
constexpr int at(const int *p, std::size_t i) { return p[i]; }
constexpr int read_past() {
  int a[4] = {1, 2, 3, 4};
  return at(a, 4);
}
constexpr int divide(int a, int b) { return a / b; }
constexpr int chain3(int b) { return divide(1, b); }
constexpr int chain2(int b) { return chain3(b) + 1; }
constexpr int chain1(int b) { return chain2(b) + 1; }
struct Holder {
  int *p = nullptr;
  constexpr int get() const { return *p; }
};
}  // namespace pfx
