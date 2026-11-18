// [expr.reflect]/1: `^^Ts...[0]` for a type pack `Ts` matches the
// `^^ type-id` form (a pack-index-specifier is a type-id); only a
// pack-index-expression is not a valid operand. kiln reflects the
// indexed type. GCC 16.1 rejects it ("'^^' cannot be applied"); see
// docs/status/deviations.md, "Reflection of a pack-index-specifier".
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
#include <meta>

template <class... Ts>
consteval std::meta::info nth() {
  return ^^Ts...[1];
}
static_assert(nth<int, const char, long>() == ^^const char);
static_assert(std::meta::is_type(nth<int, long>()));
