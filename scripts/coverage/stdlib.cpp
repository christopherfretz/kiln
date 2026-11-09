// Coverage of a libstdc++-heavy program (WP-941): containers, algorithms,
// strings, std::format, std::regex and streams. The library's template
// instantiations get no counters (clang and kiln skip functions in system
// headers), but their default arguments, temporaries and cleanups shape
// the line counts of the calling code (a default argument's location once
// made gcov count the `parse(s)` line twice). It is also the TU of the
// compile-time overhead measurement (spec/landed/codegen/WP-941.md).
#include <algorithm>
#include <format>
#include <iostream>
#include <map>
#include <numeric>
#include <optional>
#include <regex>
#include <sstream>
#include <string>
#include <unordered_map>
#include <variant>
#include <vector>

namespace {
struct Shape {
  std::variant<int, double, std::string> v;
};

std::string describe(const Shape &s) {
  return std::visit(
      [](const auto &x) -> std::string {
        if constexpr (std::is_same_v<std::decay_t<decltype(x)>, std::string>)
          return "s:" + x;
        else
          return std::format("n:{}", x);
      },
      s.v);
}

std::optional<int> parse(const std::string &s) {
  static const std::regex re("^-?[0-9]+$");
  if (!std::regex_match(s, re))
    return std::nullopt;
  return std::stoi(s);
}
} // namespace

int main(int argc, char **) {
  std::vector<int> v(50);
  std::iota(v.begin(), v.end(), -10);
  std::sort(v.begin(), v.end(), [](int a, int b) { return (a % 7) < (b % 7) || ((a % 7) == (b % 7) && a < b); });
  std::map<int, std::string> m;
  std::unordered_map<std::string, int> u;
  for (int x : v) {
    m[x % 5] += std::to_string(x);
    u[std::format("{:03}", x)]++;
  }
  std::vector<Shape> shapes{{1}, {2.5}, {std::string("t")}};
  std::ostringstream os;
  for (const auto &s : shapes)
    os << describe(s) << ';';
  int ok = 0;
  for (const char *s : {"12", "-3", "x1", "", "007"})
    ok += parse(s).value_or(-1000);
  std::cout << os.str() << m.size() << ' ' << u.size() << ' ' << ok + argc << '\n';
  return std::accumulate(v.begin(), v.end(), 0) == 975 ? 0 : 1;
}
