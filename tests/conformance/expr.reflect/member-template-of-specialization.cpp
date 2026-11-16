// [expr.reflect]/1, [temp.names]/1 (P2996R13): `^^W<int>::In` reflects
// the member class template `In` of the specialization `W<int>`; a
// nested-name-specifier naming a template specialization is followed
// like any other class qualifier.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <meta>

namespace demo {
template <class T> struct W {
  template <class U> struct In {};
};
}  // namespace demo

constexpr auto r = ^^demo::W<int>::In;
static_assert(std::meta::is_class_template(r));
static_assert(std::meta::parent_of(r) == ^^demo::W<int>);
static_assert(!std::meta::has_template_arguments(^^demo::W<int>::In));
static_assert(std::meta::template_of(^^demo::W<int>::In<char>) == r);
// A member type of a specialization stays a type-id.
template <class T> struct X { using type = T; static constexpr int v = 1; };
static_assert(std::meta::is_type_alias(^^X<int>::type) && dealias(^^X<int>::type) == ^^int);
static_assert(std::meta::is_variable(^^X<int>::v));

int main() {}
