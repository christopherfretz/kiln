#include <optional>
#include <variant>
#include <string>
#include <span>
#include <array>
#include <vector>
#include <tuple>
#include <expected>
struct V { int a; };
int ov_sweep(std::span<int> sp) {
  std::optional<int> o = 3;
  std::optional<std::string> os;
  os.emplace("hi");
  auto t = o.transform([](int x) { return x * 2; }).and_then([](int x) { return std::optional<long>(x); }).value_or(1);
  std::variant<int, std::string, V> v = 3;
  v = std::string("x");
  int r = std::visit([](auto&& x) -> int { if constexpr (requires { x.size(); }) return (int)x.size(); else return 0; }, v);
  r += std::holds_alternative<int>(v) + (std::get_if<V>(&v) ? 1 : 0) + (int)v.index();
  std::variant<int, double> v2 = 1.0;
  r += (int)std::get<double>(v2) + (v2 == v2) + (v2 < v2);
  std::array<int, 4> a{1, 2, 3, 4};
  std::span<int, 4> s4(a);
  std::span<const int> sc = sp;
  auto f = sp.first(1); auto l = sp.last<1>(); auto sub = sp.subspan(0, 1);
  auto by = std::as_bytes(sp);
  std::vector<int> vv{1};
  std::span<int> sv(vv);
  r += s4[0] + sc.front() + f[0] + l[0] + sub[0] + (int)by.size() + sv.back() + (int)std::size(s4);
  auto tup = std::make_tuple(1, 2.0, std::string("a"));
  auto [x, y, z] = tup;
  r += std::get<0>(tup) + x + (int)y + (int)z.size() + std::apply([](int a, double b, const std::string&) { return a + (int)b; }, tup);
  std::expected<int, std::string> e = 3;
  std::expected<int, std::string> u = std::unexpected(std::string("e"));
  r += e.value_or(0) + u.value_or(1) + e.transform([](int q) { return q + 1; }).value() + (o <=> o == 0) + *o + (int)os->size() + (int)t;
  auto opt2 = std::make_optional<V>(V{3});
  r += opt2->a + (o == 3) + (o != std::nullopt);
  return r;
}
