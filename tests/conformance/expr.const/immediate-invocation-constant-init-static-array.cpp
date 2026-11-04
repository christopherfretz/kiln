// [expr.const.imm]/1.2, [expr.const.defns]/1.6 with reflection: a
// consteval function returns a std::vector of (reflection, name) pairs,
// and std::define_static_array copies it into a static array whose span
// initializes a constexpr variable. The vector's heap is freed within
// the initializer, which is checked for constant initialization as a
// whole ([expr.const.init]/4), so the consteval call is not checked on
// its own; this holds for a plain variable, for each specialization of
// a variable template (whose non-dependent initializer is shared with
// the pattern), and for a variable template read from a lambda in
// another variable template's initializer (spec/sema.md "Immediate
// invocations"; WP-549). GCC 16.1 accepts the same file. (The plain
// variable is not inline: GCC 16.1 rejects an odr-used inline
// variable initialized this way as "not defined".)
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ stdout: 3 x yy zzz
//@ stdout: 3 1
#include <meta>
#include <vector>
extern "C" int printf(const char*, ...);
struct Entry { std::meta::info type; const char* label; };
consteval std::vector<Entry> entries() {
  std::vector<Entry> v;
  v.push_back({^^int, std::define_static_string("x")});
  v.push_back({^^long, std::define_static_string("yy")});
  v.push_back({^^char, std::define_static_string("zzz")});
  return v;
}
constexpr auto table = std::define_static_array(entries());
static_assert(table.size() == 3);
static_assert(table[1].type == ^^long);
template <std::meta::info K>
inline constexpr auto table_t = std::define_static_array(entries());
static_assert(table_t<^^int>.size() == 3);
static_assert(table_t<^^char>[2].type == ^^char);
template <std::meta::info K>
inline constexpr bool known_v = [] {
  for (Entry e : table_t<K>)
    if (e.type == K) return true;
  return false;
}();
static_assert(known_v<^^int>);
static_assert(!known_v<^^short>);
// The tables hold reflections, so they are read at run time only
// through constants taken from them.
constexpr int count = table.size();
constexpr const char* labels[] = {table[0].label, table[1].label, table[2].label};
constexpr int count_t = table_t<^^int>.size();
int main() {
  printf("%d %s %s %s\n", count, labels[0], labels[1], labels[2]);
  printf("%d %d\n", count_t, int(known_v<^^char>));
  return 0;
}
