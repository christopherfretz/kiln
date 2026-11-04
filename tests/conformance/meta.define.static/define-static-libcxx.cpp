// [meta.define.static] (P3491R3), spec/reflection-values.md §4: libc++'s
// string, string_view, vector and array as the range argument, and its
// span as define_static_array's result. P3491R3 §3.5.3's promotion of a
// vector of strings (with string_view not yet structural, the
// views are promoted as offset/length pairs).
//@ args: -stdlib=libc++ -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include <array>
#include <span>
#include <string>
#include <string_view>
#include <vector>
#include "kiln_meta.h"
using namespace std::meta;

consteval std::string build(int n) {
  std::string s;
  for (int i = 0; i < n; ++i) s += char('a' + i);
  return s;
}
constexpr const char* abc = std::define_static_string(build(3));
static_assert(abc[0] == 'a' && abc[2] == 'c' && abc[3] == '\0');
static_assert(abc == std::define_static_string(std::string_view("abc")));
static_assert(abc == std::define_static_string("abc"));

consteval std::vector<int> iota(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i) v.push_back(i);
  return v;
}
constexpr std::span<const int> sp = std::define_static_array(iota(4));
static_assert(sp.size() == 4 && sp[3] == 3);
static_assert(sp.data() == std::define_static_array(std::array{0, 1, 2, 3}).data());
static_assert(reflect_constant_array(iota(4)) == reflect_constant_array(std::array{0, 1, 2, 3}));
static_assert(std::define_static_array(std::vector<int>()).empty());

constexpr std::vector<std::string> get_strings() { return {"Jason", "Was", "Here"}; }
struct piece {
  unsigned offset, size;
};
consteval std::span<const piece> promote_strings(std::vector<std::string> vs, const char*& out) {
  std::string all;
  for (const std::string& s : vs) all += s;
  out = std::define_static_string(all);
  std::vector<piece> views;
  for (unsigned offset = 0; const std::string& s : vs) {
    views.push_back({offset, unsigned(s.size())});
    offset += s.size();
  }
  return std::define_static_array(views);
}
consteval std::string_view nth(int i) {
  const char* base = nullptr;
  auto ps = promote_strings(get_strings(), base);
  return std::string_view(base + ps[i].offset, ps[i].size);
}
static_assert(nth(0) == "Jason" && nth(1) == "Was" && nth(2) == "Here");
static_assert(nth(1).data() == nth(1).data());
