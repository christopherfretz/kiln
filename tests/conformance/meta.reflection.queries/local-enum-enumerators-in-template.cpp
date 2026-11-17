// [meta.reflection.queries] enumerators_of, [dcl.enum]/3: a block-scope
// enumeration declared opaque and then defined in a function template has
// the enumerators of its definition, once, in each instantiation, and is
// enumerable after its enumerator list. WP-1613.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <class T> consteval int count() {
  enum class J : T;
  enum class J : T { A, B, C };
  static_assert(std::meta::is_enumerable_type(^^J));
  static_assert(std::meta::enumerators_of(^^J).size() == 3);
  static_assert(std::meta::identifier_of(std::meta::enumerators_of(^^J)[2]) == "C");
  return std::meta::enumerators_of(^^J).size();
}
static_assert(count<int>() == 3 && count<unsigned char>() == 3);
