// spec/reflection.md §3.5: libc++'s vector<info> as a result, and
// vector, array, span and initializer_list of info as a reflection_range
// argument (read as data and size, by layout).
//@ args: -stdlib=libc++ -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include <array>
#include <initializer_list>
#include <span>
#include <vector>
#include "kiln_meta.h"
using namespace std::meta;
struct S {
  int m;
};
consteval std::size_t ranges() {
  std::vector<info> v{^^int, ^^S, ^^S::m};
  auto a = __kiln_reflection_range(v);
  std::array<info, 2> arr{^^int, ^^char};
  auto b = __kiln_reflection_range(arr);
  std::span<const info> sp(v);
  auto c = __kiln_reflection_range(sp);
  std::initializer_list<info> il = {^^S};
  auto d = __kiln_reflection_range(il);
  std::vector<info> e = __kiln_reflection_range(std::vector<info>{});
  return a.size() * 1000 + b.size() * 100 + c.size() * 10 + d.size() + (a[2] == ^^S::m) * 10000 +
         e.size();
}
static_assert(ranges() == 13231);
static_assert(identifier_of(^^S) == "S");
static_assert(u8identifier_of(^^S::m) == u8"m");
static_assert(source_location_of(^^S).line() == 14);
consteval bool caught() {
  try {
    (void)identifier_of(^^int);
  } catch (const std::meta::exception& x) {
    return x.from() == ^^identifier_of && std::string_view(x.what()) == "'^^int' has no identifier";
  }
  return false;
}
static_assert(caught());

int main() {}
