// [meta.reflection.substitute] (P2996R13): an argument reflection that is
// not a type, template, constant or object (a namespace, a non-static data
// member, the null reflection) makes substitute and can_substitute throw.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 30:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 30:20 note "'^^ns' cannot be a template argument"
//@ diag: 30:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 30:20 note "'^^ns' cannot be a template argument"
//@ diag: 31:20 error "call to consteval function 'std::meta::can_substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 31:20 note "'^^M::d' cannot be a template argument"
//@ diag: 31:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 31:20 note "'^^M::d' cannot be a template argument"
//@ diag: 32:20 error "call to consteval function 'std::meta::can_substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 32:20 note "'std::meta::info()' cannot be a template argument"
//@ diag: 32:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 32:20 note "'std::meta::info()' cannot be a template argument"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T> struct A {};
namespace ns {}
struct M { int d; };
constexpr info a = substitute(^^A, {^^ns});
constexpr bool b = can_substitute(^^A, {^^M::d});
constexpr bool c = can_substitute(^^A, {info{}});
int main() {}
