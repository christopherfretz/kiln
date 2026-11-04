// [meta.reflection.substitute] (P2996R13), the substitute example: fn1<int> contains
// an undeduced placeholder type, so substitute throws and r1's initializer is not a
// constant expression. Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-fail
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
//@ diag: 25:21 error "call to consteval function 'std::meta::substitute<std::initializer_list<info>>' is not a constant expression"
//@ diag: 25:21 note "'^^fn1' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ diag: 25:16 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 25:21 note "'^^fn1' cannot be substituted with these template arguments: the specialization's type contains an undeduced placeholder type"
//@ meta-header: skip the note spells the real header's default template argument initializer_list<info>
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template<class T>
  auto fn1();

static_assert(!can_substitute(^^fn1, {^^int}));  // OK
constexpr info r1 = substitute(^^fn1, {^^int});  // error: fn<int> contains an undeduced placeholder type

int main() {}
