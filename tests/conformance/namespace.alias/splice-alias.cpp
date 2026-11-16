// [namespace.alias]/1, /4: `namespace A = [: r :];` aliases the namespace
// the splice-specifier designates (a namespace, or the target of a
// namespace alias), at namespace and block scope. Found running GCC's C++
// testsuite.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#include <meta>
namespace N { constexpr int v = 1; namespace I { constexpr int w = 2; } }
namespace A = [:^^N:];
namespace B = [:^^N::I:];
namespace C = [:^^A:];
static_assert(A::v == 1 && B::w == 2 && C::v == 1);
static_assert(std::meta::is_namespace_alias(^^A));
using namespace [:^^C:];
static_assert(v == 1);
void f() {
  namespace L = [:^^N::I:];
  static_assert(L::w == 2);
}
