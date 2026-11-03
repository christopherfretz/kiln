// [meta.reflection.substitute] (P2996R13), the substitute example's fn2:
// the IS says can_substitute(^^fn2, {^^int}) is an error, since the body
// of fn2<int> must be instantiated to deduce its return type and the
// static_assert then fails. kiln, like GCC 16.1, does not instantiate the
// body: a specialization whose type still contains an undeduced placeholder
// is not substitutable (spec/landed/reflection/WP-482.md "Divergences"), so
// the program is accepted and can_substitute is false. The test pins that
// behaviour; it is compared with GCC.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta.h"
#endif
using namespace std::meta;

template<class T>
  auto fn2() {
    static_assert(^^T != ^^int);  // static assertion failed during instantiation of fn<int>
    return 0;
  }

constexpr bool r2 = can_substitute(^^fn2, {^^int});
static_assert(!r2);

int main() {}
