// [meta.reflection.member.queries]/1 (P2996R13): members_of(r, ctx)
// dealiases r first, so a namespace alias, a nested namespace's alias and
// an alias of an alias list the aliased namespace's members: the same
// reflections, whose parent_of is the namespace, not the alias. The
// alias stays an alias for the other queries ([meta.reflection.queries]):
// is_namespace and is_namespace_alias are both true, parent_of(alias) is
// the alias's own scope, dealias(alias) is the namespace (WP-1228).
// Compared with GCC 16.1.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 3 3 3 1 1
#include <meta>
#include <cstdio>
#include <string_view>

namespace leaf {
inline int echo(int v) { return v; }
struct S {};
namespace inner { inline int z = 0; }
}
namespace la = leaf;
namespace lb = la;
namespace outer { namespace na = leaf::inner; }

using namespace std::meta;
constexpr auto all = access_context::unchecked();

consteval std::size_t count(info ns) { return members_of(ns, all).size(); }
consteval bool has_named(info ns, std::string_view n) {
  for (auto m : members_of(ns, all))
    if (has_identifier(m) && identifier_of(m) == n) return true;
  return false;
}
consteval bool same_members(info a, info b) {
  auto x = members_of(a, all);
  auto y = members_of(b, all);
  if (x.size() != y.size()) return false;
  for (std::size_t i = 0; i < x.size(); ++i)
    if (x[i] != y[i] || parent_of(x[i]) != dealias(b)) return false;
  return true;
}

static_assert(has_named(^^la, "echo") && has_named(^^la, "S") && has_named(^^la, "inner"));
static_assert(count(^^la) == count(^^leaf));
static_assert(same_members(^^la, ^^leaf));
static_assert(same_members(^^lb, ^^leaf));
static_assert(same_members(^^outer::na, ^^leaf::inner));
static_assert(members_of(^^la, access_context::current()).size() == 3);
static_assert(members_of(^^la, access_context::unprivileged()).size() == 3);

// The alias itself.
static_assert(is_namespace(^^la) && is_namespace_alias(^^la));
static_assert(is_namespace(^^lb) && is_namespace_alias(^^lb));
static_assert(!is_namespace_alias(^^leaf));
static_assert(dealias(^^lb) == ^^leaf && dealias(^^outer::na) == ^^leaf::inner);
static_assert(parent_of(^^la) == ^^:: && parent_of(^^outer::na) == ^^outer);
static_assert(parent_of(^^la::echo) == ^^leaf);

int main() {
  std::printf("%zu %zu %zu %zu %zu\n", count(^^la), count(^^lb), count(^^leaf),
              count(^^outer::na), count(^^leaf::inner));
}
