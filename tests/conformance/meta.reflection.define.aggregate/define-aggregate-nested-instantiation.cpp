// [module.context], [expr.const.reflect] (CWG3162): an implicit
// instantiation that results from an evaluation has that evaluation's
// context in its instantiation context, so a constant it evaluates (the
// initializer of a variable template specialization substitute
// instantiates) sees the classes the evaluation completed with
// define_aggregate before it: complete, with their members and layout,
// and spliceable. A constant condition of an `if` in the consteval block
// is not folded early: the substitution runs when the block runs.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
struct S;
template <std::meta::info R> constexpr bool complete_v = std::meta::is_complete_type(R);
template <std::meta::info R>
constexpr std::size_t members_v =
    std::meta::nonstatic_data_members_of(R, std::meta::access_context::unchecked()).size();
template <std::meta::info R> constexpr std::size_t size_v = std::meta::size_of(R);
template <std::meta::info R> constexpr std::size_t splice_v = sizeof(typename[:R:]);
consteval std::meta::info at(std::meta::info templ, std::meta::info cls) {
  return std::meta::substitute(templ, {std::meta::reflect_constant(cls)});
}
consteval {
  std::meta::define_aggregate(^^S, {std::meta::data_member_spec(^^int, {.name = "x"}),
                                    std::meta::data_member_spec(^^int, {.name = "y"})});
  if (!std::meta::extract<bool>(at(^^complete_v, ^^S))) throw 1;
  if (std::meta::extract<std::size_t>(at(^^members_v, ^^S)) != 2) throw 2;
  if (std::meta::extract<std::size_t>(at(^^size_v, ^^S)) != 2 * sizeof(int)) throw 3;
  if (std::meta::extract<std::size_t>(at(^^splice_v, ^^S)) != 2 * sizeof(int)) throw 4;
}
static_assert(complete_v<^^S> && members_v<^^S> == 2 && size_v<^^S> == 2 * sizeof(int));
