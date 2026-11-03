// [meta.reflection.substitute] (P2996R13), the substitute example:
// to_integral_constant forms integral_constant<unsigned, i> from a
// reflected constant; and fn1<int> (an undeduced placeholder return type,
// no definition) cannot be substituted. Compared with GCC 16.1
// (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
#if __has_include(<meta>)
#include <meta>
#include <type_traits>
#else
#include "kiln_meta.h"
namespace std {
template <class T, T v> struct integral_constant { static constexpr T value = v; };
}
#endif
using namespace std::meta;

template<class T>
  auto fn1();

static_assert(!can_substitute(^^fn1, {^^int}));  // OK

consteval info to_integral_constant(unsigned i) {
  return substitute(^^std::integral_constant, {^^unsigned, reflect_constant(i)});
}
constexpr info r = to_integral_constant(2);  // OK, r represents the type
                                             // std::integral_constant<unsigned, 2>
static_assert(r == ^^std::integral_constant<unsigned, 2>);

int main() {}
