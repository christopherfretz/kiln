// [meta.reflection.substitute], [meta.reflection.define.aggregate]
// (P2996R13): substitute() of an alias template whose type is a member of
// a class template specialization instantiates that specialization (not
// in the immediate context of the substitution), and a consteval block of
// it may complete a class with define_aggregate. Found running GCC's C++
// testsuite (the parse_json example of P2996).
//@ xfail(kiln): WP-1614: define_aggregate is refused while std::meta::substitute is trying a substitution, even outside its immediate context
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
consteval std::meta::info make() {
  auto m = std::meta::data_member_spec(^^int, {.name = "x"});
  return std::meta::substitute(^^Cls, {std::meta::reflect_constant(m)});
}
using T = [:make():];
static_assert(T{5}.x == 5);
