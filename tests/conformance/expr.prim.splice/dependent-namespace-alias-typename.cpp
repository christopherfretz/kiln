// [temp.dep.namespace]/2's example (P2996R13): `typename Alias::T{}` with
// `Alias` a dependent namespace alias names the type alias of the
// namespace the substituted splice designates; `Alias::In::w` a member of
// its nested namespace. WP-1613.
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 has an internal compiler error on it (make_typename_type)
//@ expect: compile-ok
template <decltype(^^int) R> constexpr int fn() {
  namespace Alias = [:R:];
  return typename Alias::T{} + sizeof(typename Alias::T) + Alias::In::w;
}
namespace NS { using T = int; namespace In { constexpr int w = 100; } }
namespace NL { using T = long; namespace In { constexpr int w = 200; } }
static_assert(fn<^^NS>() == sizeof(int) + 100);
static_assert(fn<^^NL>() == sizeof(long) + 200);
