// [meta.reflection.substitute] (P2996R13): substitute of a reflection that
// is not a template, and template_of / template_arguments_of of one that is
// not a specialization (including an alias of a specialization), throw.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 33:20 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 33:20 note "'^^int' is not a template"
//@ diag: 33:16 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 33:20 note "'^^int' is not a template"
//@ diag: 34:20 error "call to consteval function 'std::meta::template_of' is not a constant expression"
//@ diag: 34:20 note "'^^int' is not a template specialization"
//@ diag: 34:16 error "constexpr variable 'b' must be initialized by a constant expression"
//@ diag: 34:20 note "'^^int' is not a template specialization"
//@ diag: 35:20 error "call to consteval function 'std::meta::template_of' is not a constant expression"
//@ diag: 35:20 note "'^^AI' is not a template specialization"
//@ diag: 35:16 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 35:20 note "'^^AI' is not a template specialization"
//@ diag: 36:20 error "call to consteval function 'std::meta::template_arguments_of' is not a constant expression"
//@ diag: 36:20 note "'^^A' is not a template specialization"
//@ diag: 36:16 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 36:20 note "'^^A' is not a template specialization"
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T> struct A {};
using AI = A<int>;
constexpr info a = substitute(^^int, {^^int});
constexpr info b = template_of(^^int);
constexpr info c = template_of(^^AI);
constexpr auto d = template_arguments_of(^^A).size();
int main() {}
