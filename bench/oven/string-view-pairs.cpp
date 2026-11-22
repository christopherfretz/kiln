//@ args: -fconstexpr-steps=2000000000
// Standard-library plumbing in a hot loop (reduced from real code, WP-1864):
// N generated names with a shared prefix, then every pair compared with
// std::string_view ==, through libstdc++'s char_traits<char>::compare, its
// is_constant_evaluated wrapper and std::string's small-buffer union.
// Measures per-call overhead of tiny library functions (spec/oven.md
// "Library fast paths", docs/status/oven-profile.md).
#include <cstddef>
#include <string>
#include <string_view>
#include <vector>

consteval std::size_t equal_pairs(std::size_t n) {
  std::vector<std::string> names;
  for (std::size_t i = 0; i < n; ++i) {
    std::string s = "field_";
    for (std::size_t v = i;; v /= 10) {
      s.push_back(static_cast<char>('0' + v % 10));
      if (v < 10) break;
    }
    names.push_back(s);
  }
  std::vector<std::string_view> ids(names.begin(), names.end());
  std::size_t hits = 0;
  for (std::size_t a = 0; a < n; ++a)
    for (std::size_t b = a + 1; b < n; ++b)
      hits += ids[a] == ids[b];
  return hits;
}

static_assert(equal_pairs(1000) == 0);
