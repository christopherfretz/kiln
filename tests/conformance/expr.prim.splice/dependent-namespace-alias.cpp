// [temp.dep.namespace] (P2996R13): a namespace alias whose
// splice-specifier is dependent is a dependent namespace alias, resolved
// in each instantiation. Found running GCC's C++ testsuite.
//@ xfail(kiln): WP-1613: a namespace alias of a dependent splice-specifier is not supported
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
template <decltype(^^int) R> constexpr int fn() {
  namespace Alias = [:R:];
  return Alias::v;
}
namespace NS { int v = 3; }
namespace NT { constexpr int v = 4; }
int a = fn<^^NS>();
static_assert(fn<^^NT>() == 4);
