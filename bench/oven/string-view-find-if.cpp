//@ args: -fconstexpr-steps=2000000000
// Lookup by name over a std::vector<std::string_view> with iterators,
// std::find_if and std::equal_to<> (reduced from real code, WP-1864): each
// query walks the table comparing string_views, so the time is per-call
// library plumbing (iterator operators, the predicate, char_traits).
#include <algorithm>
#include <cstddef>
#include <functional>
#include <string_view>
#include <vector>

constexpr std::string_view words[] = {
    "alpha", "bravo", "charlie", "delta", "echo", "foxtrot", "golf", "hotel",
    "india", "juliett", "kilo", "lima", "mike", "november", "oscar", "papa",
    "quebec", "romeo", "sierra", "tango", "uniform", "victor", "whiskey",
    "xray", "yankee", "zulu"};

consteval std::size_t lookups(std::size_t rounds) {
  std::vector<std::string_view> table;
  for (std::string_view w : words) table.push_back(w);
  for (std::string_view w : words) table.push_back(w.substr(1));
  std::size_t sum = 0;
  for (std::size_t r = 0; r < rounds; ++r)
    for (std::string_view q : words) {
      auto it = std::find_if(table.begin(), table.end(), [q](std::string_view x) {
        return std::equal_to<>{}(x.substr(1), q.substr(1)) && x.size() == q.size();
      });
      sum += static_cast<std::size_t>(it - table.begin());
    }
  return sum;
}

static_assert(lookups(1) == 325);
static_assert(lookups(800) == 800 * 325);
