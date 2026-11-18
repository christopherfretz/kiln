// [meta.reflection.substitute], [expr.const.reflect]: can_substitute
// forms the same template-id substitute does, so a class template
// specialization it instantiates (outside the substitution's immediate
// context) runs its consteval block, which may complete a class with
// define_aggregate: can_substitute is true and the class is complete
// afterwards; substitute of the same arguments then succeeds and names
// the same class, and of other arguments it completes another one.
// GCC 16.1 agrees; nothing in the text exempts can_substitute.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <std::meta::info... Ms> struct Outer {
  struct Inner;
  consteval { std::meta::define_aggregate(^^Inner, {Ms...}); }
};
template <std::meta::info... Ms> using Cls = Outer<Ms...>::Inner;
consteval std::meta::info member(std::meta::info t, const char *n) {
  return std::meta::reflect_constant(std::meta::data_member_spec(t, {.name = n}));
}
consteval bool probe() { return std::meta::can_substitute(^^Cls, {member(^^int, "x")}); }
static_assert(probe());
static_assert(std::meta::is_complete_type(^^Outer<std::meta::data_member_spec(^^int, {.name = "x"})>::Inner));
consteval std::meta::info make(std::meta::info t, const char *n) {
  return std::meta::substitute(^^Cls, {member(t, n)});
}
static_assert(std::meta::dealias(make(^^int, "x")) == ^^Outer<std::meta::data_member_spec(^^int, {.name = "x"})>::Inner);
static_assert(std::meta::can_substitute(^^Cls, {member(^^long, "y")}));
static_assert(std::meta::size_of(make(^^long, "y")) == sizeof(long));
using T = [:make(^^int, "x"):];
static_assert(T{3}.x == 3);
// A substitution that fails in its immediate context is false (and
// substitute throws).
template <class T, std::meta::info... Ms>
  requires(sizeof(T) == 1)
using Pick = Outer<Ms...>::Inner;
static_assert(!std::meta::can_substitute(^^Pick, {^^int, member(^^short, "z")}));
consteval bool throws() {
  try {
    std::meta::substitute(^^Pick, {^^int, member(^^short, "z")});
  } catch (std::meta::exception &) {
    return true;
  }
  return false;
}
static_assert(throws());
