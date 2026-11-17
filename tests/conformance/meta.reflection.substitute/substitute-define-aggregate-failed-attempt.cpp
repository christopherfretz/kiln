// [meta.reflection.substitute], [temp.alias]: a substitution that fails
// in its immediate context (a constraint of the alias template) throws
// before it instantiates the class template, so the consteval block that
// would complete the class with define_aggregate does not run; a later
// substitution that succeeds completes it once.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <std::meta::info... Ms> struct Outer {
  struct Inner;
  consteval { std::meta::define_aggregate(^^Inner, {Ms...}); }
};
template <class T, std::meta::info... Ms>
  requires(sizeof(T) == 1)
using Pick = Outer<Ms...>::Inner;
consteval std::meta::info attempt(std::meta::info t) {
  auto m = std::meta::data_member_spec(^^int, {.name = "x"});
  try {
    return std::meta::substitute(^^Pick, {t, std::meta::reflect_constant(m)});
  } catch (std::meta::exception &) {
    return std::meta::info{};
  }
}
static_assert(attempt(^^int) == std::meta::info{});
static_assert(attempt(^^char) != std::meta::info{});
static_assert(std::meta::is_complete_type(attempt(^^char)));
static_assert(std::meta::nonstatic_data_members_of(
                  attempt(^^char), std::meta::access_context::unchecked())
                  .size() == 1);
