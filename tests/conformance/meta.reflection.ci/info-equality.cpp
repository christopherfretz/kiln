// [basic.fundamental]/17, [expr.eq]/6 (P2996R13): two std::meta::info
// values compare equal iff they represent the same entity (types compared
// with their cv-qualifiers, a null reflection equal only to another null
// one), whether produced by `^^`, a consteval function, or a template
// argument. Reflections are consteval-only ([basic.types.general]/12), so
// each comparison is folded into a constexpr bool before runtime use.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ stdout: 1000101001010101010
#include <cstdio>
using info = decltype(^^int);
namespace N { int v; void f(); struct S { int a; int b; }; namespace Inner {} }
template <info R> struct Tag { static constexpr info value = R; };
consteval info pick(bool b) { return b ? ^^int : ^^char; }
constexpr info null{};
constexpr bool eq[] = {
  ^^int == ^^int, ^^int == ^^long, ^^const int == ^^int, ^^int* == ^^int,
  ^^N == ^^N, ^^N::Inner == ^^N, ^^N::v == ^^N::v, ^^N::f == ^^N::v,
  ^^N::S::a == ^^N::S::b, ^^N::S == ^^N::S, ^^N::S == ^^N::S::a,
  Tag<^^int>::value == ^^int, Tag<^^char>::value == Tag<^^int>::value,
  pick(true) == (^^int), pick(false) == (^^int), null == info{}, null == ^^int,
  ^^void (*)(int) == ^^void (*)(int), ^^N::S::a != ^^N::S::a,
};
int main() {
  for (bool b : eq) std::printf("%d", b);
  std::printf("\n");
}
