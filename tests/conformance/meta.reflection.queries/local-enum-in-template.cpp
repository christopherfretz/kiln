// [dcl.enum]/3, [temp.inst] (P2996R13 is_enumerable_type): a local opaque
// enum-declaration followed by its definition in a function template is
// one enumeration in each instantiation; is_enumerable_type is false
// inside its own enumerator list and true after it. Found running GCC's
// C++ testsuite.
//@ xfail(kiln): WP-1613: instantiating a local opaque enum then its definition redefines its enumerators (templ/)
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <class T> int g() {
  enum J : T;
  enum J : T { D = std::meta::is_enumerable_type(^^J) ? 2 : 1 };
  static_assert(D == 1);
  static_assert(std::meta::is_enumerable_type(^^J));
  return D;
}
template int g<int>();
