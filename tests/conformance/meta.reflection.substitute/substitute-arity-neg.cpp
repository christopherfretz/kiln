// [meta.reflection.substitute] (P2996R13): substitute with too few or too many template
// arguments throws std::meta::exception; uncaught, the initializer is not a constant expression.
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 24:22 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 24:22 note "'^^Pair' cannot be substituted with these template arguments: wrong number of template arguments (too few template arguments for class template 'Pair')"
//@ diag: 24:16 error "constexpr variable 'few' must be initialized by a constant expression"
//@ diag: 24:22 note "'^^Pair' cannot be substituted with these template arguments: wrong number of template arguments (too few template arguments for class template 'Pair')"
//@ diag: 25:23 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 25:23 note "'^^Pair' cannot be substituted with these template arguments: wrong number of template arguments (too many template arguments for class template 'Pair')"
//@ diag: 25:16 error "constexpr variable 'many' must be initialized by a constant expression"
//@ diag: 25:23 note "'^^Pair' cannot be substituted with these template arguments: wrong number of template arguments (too many template arguments for class template 'Pair')"
//@ meta-header: skip the note spells the real header's default template argument initializer_list<info>
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;
template <class T, class U = T> struct Pair {};
constexpr info few = substitute(^^Pair, {});
constexpr info many = substitute(^^Pair, {^^int, ^^int, ^^int});
int main() {}
