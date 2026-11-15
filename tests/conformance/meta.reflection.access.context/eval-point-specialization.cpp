// [meta.reflection.access.context], [meta.reflection.scope]: the
// initializer of a static data member of a class template specialization
// is evaluated in the scope of the specialization, so
// access_context::current() there names S<int>, not the template. Found
// running GCC's C++ testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
//@ xfail(kiln): WP-1482, the non-dependent immediate invocation is evaluated once in the template, scope ^^S
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

template <typename T>
struct S {
  static constexpr auto ctx = access_context::current();
};
static_assert(S<int>::ctx.scope() == ^^S<int>);
