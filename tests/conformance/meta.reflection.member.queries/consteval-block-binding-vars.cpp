// [meta.reflection.member.queries]/2: a consteval block declares no
// member ([dcl.pre]: it is a static_assert-declaration's sibling), while
// a tuple-like structured binding declaration at namespace scope
// introduces the variable it decomposes and one reference variable r_i
// per binding ([dcl.struct.bind]/7), all members; the bindings
// themselves are not. Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
#include <ranges>
#include <tuple>
#include <vector>
using namespace std::meta;

struct P { int v; template <int I> int &get() { return v; } };
template <> struct std::tuple_size<P> { static const int value = 3; };
template <std::size_t I> struct std::tuple_element<I, P> { using type = int; };

namespace A {
  struct S { int m; consteval {} };
  consteval {}
}
namespace B { auto [x, y, z] = P{}; }
namespace C { int arr[2]; auto [p, q] = arr; }

constexpr auto ctx = access_context::unchecked();
static_assert(members_of(^^A, ctx).size() == 1);
static_assert(nonstatic_data_members_of(^^A::S, ctx).size() == 1);
static_assert(members_of(^^B, ctx).size() == 4);
static_assert((members_of(^^B, ctx) | std::views::filter(is_variable)
               | std::ranges::to<std::vector>()).size() == 4);
static_assert((members_of(^^B, ctx) | std::views::filter(is_structured_binding)
               | std::ranges::to<std::vector>()).empty());
static_assert((members_of(^^B, ctx) | std::views::filter(has_identifier)
               | std::ranges::to<std::vector>()).size() == 3);
// Not tuple-like: the bindings name the array's elements; only the
// unnamed variable is a member.
static_assert(members_of(^^C, ctx).size() == 2);
