//@ mode: emit-llvm
//@ args: -g
// A format-test-like TU at `-g` (WP-817): many small test functions, each
// formatting a few values through `std::format`, so the unit carries the
// debug information of a library-heavy TU (deep class templates from
// <format>, <string>, <vector> and <map>, many subprograms, locals and
// locations). Tracks the front-end cost of `-g`
// (docs/status/debuginfo.md "Cost of `-g`").
#include <format>
#include <map>
#include <string>
#include <string_view>
#include <vector>

struct Point { int x, y; };

template <> struct std::formatter<Point> : std::formatter<int> {
  auto format(const Point &p, std::format_context &ctx) const {
    auto out = std::formatter<int>::format(p.x, ctx);
    *out++ = ',';
    ctx.advance_to(out);
    return std::formatter<int>::format(p.y, ctx);
  }
};

static int failures = 0;
static void check(const std::string &got, std::string_view want) {
  if (got != want) ++failures;
}

#define KILN_T(n)                                                          \
  void test_##n() {                                                        \
    int i = n;                                                             \
    double d = n * 0.5;                                                    \
    std::string s = std::to_string(n);                                     \
    std::vector<int> v{n, n + 1, n + 2};                                   \
    std::map<int, std::string> m{{n, s}};                                  \
    check(std::format("{}", i), s);                                        \
    check(std::format("{:>8}|{:<6}|{:^5}", i, s, 'c'), s);                 \
    check(std::format("{:.3f} {:e} {:g}", d, d, d), s);                    \
    check(std::format("{:#x} {:o} {:b}", i, i, unsigned(i)), s);           \
    check(std::format("{} {}", Point{i, i + 1}, v.size()), s);             \
    check(std::format("{}:{}", m.begin()->first, m.begin()->second), s);   \
  }
#define KILN_T10(d) KILN_T(d##0) KILN_T(d##1) KILN_T(d##2) KILN_T(d##3) \
  KILN_T(d##4) KILN_T(d##5) KILN_T(d##6) KILN_T(d##7) KILN_T(d##8) KILN_T(d##9)
KILN_T10(1) KILN_T10(2) KILN_T10(3) KILN_T10(4) KILN_T10(5)
KILN_T10(6) KILN_T10(7) KILN_T10(8) KILN_T10(9)

int main() {
#define KILN_C(n) test_##n();
#define KILN_C10(d) KILN_C(d##0) KILN_C(d##1) KILN_C(d##2) KILN_C(d##3) \
  KILN_C(d##4) KILN_C(d##5) KILN_C(d##6) KILN_C(d##7) KILN_C(d##8) KILN_C(d##9)
  KILN_C10(1) KILN_C10(2) KILN_C10(3) KILN_C10(4) KILN_C10(5)
  KILN_C10(6) KILN_C10(7) KILN_C10(8) KILN_C10(9)
  return failures;
}
