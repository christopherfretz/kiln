// Constant-evaluation state carried by a snapshot (WP-1396): compiled
// bytecode, memoized calls, globals and their static objects, virtual
// dispatch, exceptions, transient allocations, unions and member pointers
// evaluated in the prefix, then used again by the main file.
#include <array>
#include <vector>
#include <string_view>
#include <memory>
#include <algorithm>
#include <stdexcept>
#include <optional>
#include <variant>
namespace pfx {
constexpr unsigned long fib(unsigned n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
inline constexpr unsigned long fib22 = fib(22);
constexpr std::array<int, 64> squares = [] {
  std::array<int, 64> a{};
  for (int i = 0; i < 64; ++i) a[i] = i * i;
  return a;
}();
struct Shape {
  constexpr virtual ~Shape() = default;
  constexpr virtual int area() const = 0;
};
struct Rect : Shape {
  int w, h;
  constexpr Rect(int w, int h) : w(w), h(h) {}
  constexpr int area() const override { return w * h; }
};
struct Square final : Rect {
  constexpr explicit Square(int s) : Rect(s, s) {}
};
constexpr int total_area(int n) {
  std::vector<std::unique_ptr<Shape>> v;
  for (int i = 1; i <= n; ++i) {
    if (i % 2) v.push_back(std::make_unique<Rect>(i, i + 1));
    else v.push_back(std::make_unique<Square>(i));
  }
  int t = 0;
  for (auto &s : v) t += s->area();
  return t;
}
static_assert(total_area(10) == 410);
constexpr int checked_div(int a, int b) {
  if (b == 0) throw std::domain_error("divide by zero");
  return a / b;
}
constexpr int try_div(int a, int b) {
  try {
    return checked_div(a, b);
  } catch (const std::exception &) {
    return -1;
  }
}
static_assert(try_div(7, 0) == -1);
union Word {
  int i;
  float f;
};
constexpr int active_int(int v) {
  Word w{.i = v};
  return w.i;
}
struct Pair {
  int a, b;
};
constexpr int pick(int Pair::*m, Pair p) { return p.*m; }
static_assert(pick(&Pair::b, {1, 2}) == 2);
constexpr std::string_view words[] = {"alpha", "beta", "gamma", "delta"};
constexpr std::size_t longest() {
  std::size_t n = 0;
  for (auto w : words) n = std::max(n, w.size());
  return n;
}
inline constexpr std::size_t longest_word = longest();
constexpr std::optional<int> find(int x) {
  for (int i = 0; i < 64; ++i)
    if (squares[i] == x) return i;
  return std::nullopt;
}
using V = std::variant<int, long, double>;
constexpr long as_long(V v) {
  return std::visit([](auto x) { return static_cast<long>(x); }, v);
}
template <int N> struct Counter {
  static constexpr int value = Counter<N - 1>::value + N;
};
template <> struct Counter<0> {
  static constexpr int value = 0;
};
inline constexpr int counted = Counter<200>::value;
}  // namespace pfx
