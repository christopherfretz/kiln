// [meta.reflection.substitute] (P2996R13): an argument of the wrong kind (a
// type for a constant or template template parameter, a constant for a type
// parameter) makes substitute throw.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 30:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 30:20 note "'^^A' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for template type parameter must be a type)"
//@ diag: 30:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 30:20 note "'^^A' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for template type parameter must be a type)"
//@ diag: 31:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 31:20 note "'^^TTa' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for template template parameter must be a class template or type alias template)"
//@ diag: 31:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 31:20 note "'^^TTa' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for template template parameter must be a class template or type alias template)"
//@ diag: 32:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 32:20 note "'^^P' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for non-type template parameter must be an expression)"
//@ diag: 32:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 32:20 note "'^^P' cannot be substituted with these template arguments: template argument of the wrong kind (template argument for non-type template parameter must be an expression)"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T> struct A {};
template <template <class> class TT> struct TTa {};
template <auto... V> struct P {};
constexpr info a = substitute(^^A, {template_arguments_of(^^P<1>)[0]});
constexpr info b = substitute(^^TTa, {^^int});
constexpr info c = substitute(^^P, {^^int});
int main() {}
