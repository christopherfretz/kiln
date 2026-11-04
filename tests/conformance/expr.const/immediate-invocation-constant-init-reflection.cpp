// [expr.const.imm]/1.2, [expr.const.defns]/1.6 with reflection: a
// consteval function collects reflections from members_of and
// template_arguments_of into a std::vector<std::meta::info> member of
// its result and returns it; an inline constexpr variable template (and
// a constexpr variable) takes only the trivially-copyable summary member.
// The call is validated by the variable's constant-initialization check
// ([expr.const.init]/4), during which the vector is destroyed, so it is
// accepted (spec/sema.md "Immediate invocations"; WP-549). GCC 16.1
// accepts the same file.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <vector>
namespace ns {
struct A { int x; long y; };
template <class T, int N> struct Box {};
}
struct Summary { int members = 0; int args = 0; bool empty = false; };
struct Plan { std::vector<std::meta::info> entries; Summary sum{}; };
template <std::meta::info... Rs>
consteval Plan collect() {
  Plan p;
  constexpr auto ctx = std::meta::access_context::unchecked();
  for (std::meta::info r : {Rs...}) {
    if (std::meta::has_template_arguments(r)) {
      for (std::meta::info a : std::meta::template_arguments_of(r)) {
        p.entries.push_back(a);
        ++p.sum.args;
      }
    } else {
      for (std::meta::info m : std::meta::members_of(r, ctx)) {
        if (!std::meta::is_nonstatic_data_member(m)) continue;
        p.entries.push_back(m);
        ++p.sum.members;
      }
    }
  }
  p.sum.empty = p.entries.empty();
  return p;
}
template <std::meta::info... Rs>
inline constexpr Summary summary_v = collect<Rs...>().sum;
static_assert(summary_v<^^ns::A>.members == 2);
static_assert(summary_v<^^ns::A, ^^ns::Box<int, 3>>.args == 2);
static_assert(summary_v<^^ns::A, ^^ns::Box<int, 3>>.members == 2);
static_assert(!summary_v<^^ns::A>.empty);
constexpr Summary direct = collect<^^ns::Box<char, 1>>().sum;
static_assert(direct.args == 2 && direct.members == 0);
int main() { return summary_v<^^ns::A>.members + direct.args - 4; }
