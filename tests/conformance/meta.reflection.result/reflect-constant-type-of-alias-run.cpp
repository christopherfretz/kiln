// [meta.reflection.result]/2-4, [meta.reflection.queries]/2.2 (type_of),
// /54 (dealias), /59.1 and /59.3 (template_arguments_of), [expr.reflect]/5.6.2,
// [expr.eq]/7.4, [basic.pre]/8, [temp.param]/11 and /13, [meta.syn]
// (spec/reflection.md §3 "The type of a reflected constant", WP-1255):
// `type_of(reflect_constant(^^int)) == ^^std::meta::info` is false by the
// standard. `std::meta::info` is the type alias `using info =
// decltype(^^::);`, and `^^std::meta::info` reflects that alias, an entity
// distinct from its underlying type. `reflect_constant` of a non-class `T`
// reflects a value, whose type is `T` (cv-unqualified, an alias's underlying
// type, never the alias). So type_of yields `decltype(^^::)`, equal to
// `dealias(^^std::meta::info)`. A class type's result is a template parameter
// object of type `const T` instead. The output is compared with GCC 16.1's.
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: info 0 0 1 1 1 0 0 1 / S 0 1 1 1 / int 1 0 1 0 1
#include <cstdio>
#include <meta>
#include <type_traits>
using namespace std::meta;

struct S { int x; };
using Alias = int;

constexpr info ri = reflect_constant(^^int);
constexpr info rs = reflect_constant(S{1});
constexpr info r42 = reflect_constant(42);

// A value of type info: its type is the reflection type, not the alias.
static_assert(is_value(ri) && !is_object(ri));
static_assert(type_of(ri) != ^^std::meta::info);
static_assert(type_of(ri) != ^^info);
static_assert(is_type_alias(^^std::meta::info));
static_assert(!is_type_alias(type_of(ri)));
static_assert(type_of(ri) == dealias(^^std::meta::info));
static_assert(dealias(type_of(ri)) == dealias(^^std::meta::info));
static_assert(type_of(ri) == ^^decltype(^^::));
static_assert(!is_const_type(type_of(ri)));
static_assert(remove_cv(type_of(ri)) == type_of(ri));
static_assert(std::is_same_v<typename[:type_of(ri):], std::meta::info>);
static_assert(type_of(reflect_constant(^^Alias)) == dealias(^^info));

// A class type: a template parameter object of type const S.
static_assert(is_object(rs));
static_assert(type_of(rs) != ^^S);
static_assert(type_of(rs) == ^^const S);
static_assert(is_const_type(type_of(rs)));
static_assert(remove_cv(type_of(rs)) == ^^S);

// An int value: cv-unqualified, and an alias argument reflects as int.
static_assert(is_value(r42));
static_assert(type_of(r42) == ^^int);
static_assert(type_of(r42) != ^^Alias);
static_assert(type_of(r42) == dealias(^^Alias));
static_assert(!is_const_type(type_of(r42)));
static_assert(type_of(reflect_constant(Alias{7})) == ^^int);

// Each query runs in a constant expression; main prints the results.
#define B(...) ([] { constexpr bool b_ = (__VA_ARGS__); return b_ ? 1 : 0; }())

int main() {
  std::printf("info %d %d %d %d %d %d %d %d / ",
              B(type_of(ri) == ^^std::meta::info),
              B(is_type_alias(type_of(ri))),
              B(type_of(ri) == dealias(^^std::meta::info)),
              B(dealias(type_of(ri)) == dealias(^^std::meta::info)),
              B(type_of(ri) == ^^decltype(^^::)),
              B(is_const_type(type_of(ri))),
              B(is_object(ri)),
              B(std::is_same_v<typename[:type_of(ri):], std::meta::info>));
  std::printf("S %d %d %d %d / ",
              B(type_of(rs) == ^^S),
              B(type_of(rs) == ^^const S),
              B(is_const_type(type_of(rs))),
              B(remove_cv(type_of(rs)) == ^^S));
  std::printf("int %d %d %d %d %d\n",
              B(type_of(r42) == ^^int),
              B(type_of(r42) == ^^Alias),
              B(type_of(r42) == dealias(^^Alias)),
              B(is_const_type(type_of(r42))),
              B(is_value(r42)));
}
