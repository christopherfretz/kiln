// [meta.reflection.substitute] (P2996R13): a substitution failure in an
// alias template's type makes substitute throw; it is a SFINAE context, so
// nothing else is diagnosed.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 26:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 26:20 note "'^^Bad' cannot be substituted with these template arguments: substitution failure (type 'int' cannot be used prior to '::' because it has no members)"
//@ diag: 26:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 26:20 note "'^^Bad' cannot be substituted with these template arguments: substitution failure (type 'int' cannot be used prior to '::' because it has no members)"
//@ diag: 27:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 27:20 note "'^^Ptr' cannot be substituted with these template arguments: substitution failure ('type name' declared as a pointer to a reference of type 'int &')"
//@ diag: 27:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 27:20 note "'^^Ptr' cannot be substituted with these template arguments: substitution failure ('type name' declared as a pointer to a reference of type 'int &')"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T> using Bad = typename T::type;
template <class T> using Ptr = T*;
static_assert(!can_substitute(^^Bad, {^^int}));
constexpr info a = substitute(^^Bad, {^^int});
constexpr info b = substitute(^^Ptr, {^^int&});
int main() {}
