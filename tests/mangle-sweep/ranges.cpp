#include <ranges>
#include <vector>
#include <string>
#include <string_view>
#include <map>
int ranges_sweep(std::vector<int>& v, std::map<int, std::string>& m) {
  int r = 0;
  for (int x : v | std::views::filter([](int i) { return i % 2; }) | std::views::transform([](int i) { return i * 2; })) r += x;
  for (int x : std::views::iota(0, 5)) r += x;
  for (int x : v | std::views::take(2) | std::views::drop(1) | std::views::reverse) r += x;
  for (auto&& k : m | std::views::keys) r += k;
  for (auto&& s : m | std::views::values) r += (int)s.size();
  for (auto [a, b] : std::views::zip(v, v)) r += a * b;
  for (auto w : std::string_view("a b c") | std::views::split(' ')) r += (int)std::ranges::distance(w);
  for (int x : v | std::views::take_while([](int i) { return i < 3; })) r += x;
  for (int x : v | std::views::drop_while([](int i) { return i < 3; })) r += x;
  for (int x : std::views::single(3)) r += x;
  for (int x : std::views::repeat(1, 3)) r += x;
  std::vector<std::vector<int>> vv{{1}, {2}};
  for (int x : vv | std::views::join) r += x;
  for (int x : v | std::views::common) r += x;
  for (auto x : std::views::counted(v.begin(), 1)) r += x;
  auto vec = v | std::ranges::to<std::vector<long>>();
  r += (int)vec.size();
  auto e = std::ranges::empty_view<int>{};
  r += (int)e.size();
  return r;
}
