// fmt_module (WP-1148, fmt-module-tests.cmake): a consumer of the {fmt}
// named module that includes no {fmt} header. Exits non-zero, naming the
// case, when a formatted string differs from the expected text.
#include <chrono>
#include <cstdio>
#include <iterator>
#include <map>
#include <string>
#include <string_view>
#include <vector>

import fmt;

namespace {
int failures = 0;

void check(std::string_view what, const std::string& got, std::string_view want) {
  if (got != want) {
    std::printf("FAIL %.*s: got '%s', want '%.*s'\n", int(what.size()), what.data(), got.c_str(),
                int(want.size()), want.data());
    ++failures;
  }
}

struct point {
  int x, y;
};
}  // namespace

template <> struct fmt::formatter<point> : fmt::formatter<int> {
  auto format(point p, fmt::format_context& ctx) const {
    return fmt::format_to(ctx.out(), "({}, {})", p.x, p.y);
  }
};

int main() {
  check("format", fmt::format("{} + {} = {}", 1, 2, 1 + 2), "1 + 2 = 3");
  check("spec", fmt::format("{:>6.2f}|{:#x}|{:08b}", 3.14159, 255, 5), "  3.14|0xff|00000101");
  check("named", fmt::format("{a}-{b}", fmt::arg("a", "x"), fmt::arg("b", 7)), "x-7");
  check("join", fmt::format("{}", fmt::join(std::vector<int>{1, 2, 3}, ", ")), "1, 2, 3");
  check("ranges", fmt::format("{}", std::map<std::string, int>{{"a", 1}, {"b", 2}}),
        "{\"a\": 1, \"b\": 2}");
  check("chrono", fmt::format("{}", std::chrono::seconds(42)), "42s");
  check("user formatter", fmt::format("{}", point{3, -4}), "(3, -4)");
  fmt::memory_buffer buf;
  fmt::format_to(std::back_inserter(buf), "{}{}", "ab", 'c');
  check("memory_buffer", fmt::to_string(buf), "abc");
  check("to_string", fmt::to_string(12345), "12345");
  check("format_int", std::string(fmt::format_int(-42).c_str()), "-42");
  check("runtime", fmt::format(fmt::runtime("{}/{}"), 6, 7), "6/7");
  check("formatted_size", fmt::to_string(fmt::formatted_size("{}", 1000)), "4");
  check("styled", fmt::format("{}", fmt::styled(1, fmt::emphasis::bold)), "\x1b[1m1\x1b[0m");
  try {
    (void)fmt::format(fmt::runtime("{"), 1);
    check("format_error", "no throw", "throw");
  } catch (const fmt::format_error&) {
  }
  if (failures == 0) fmt::print("fmt module smoke: 14 cases ok\n");
  return failures == 0 ? 0 : 1;
}
