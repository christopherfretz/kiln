// [meta.reflection.access.context], [meta.reflection.scope]: the
// initializer of a static data member of a class template specialization
// is evaluated in the scope of the specialization, so
// access_context::current() there names S<int>, not the template, also
// when the call is a non-dependent part of a dependent initializer; each
// specialization evaluates it at its own point. Found running GCC's C++
// testsuite.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
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
static_assert(S<long>::ctx.scope() == ^^S<long>);

template <typename T>
struct D {
  static constexpr auto ctx = ((void)sizeof(T), access_context::current());
};
static_assert(D<int>::ctx.scope() == ^^D<int>);
static_assert(D<char>::ctx.scope() == ^^D<char>);

template <typename T>
struct F {
  static consteval info f() { return access_context::current().scope(); }
};
static_assert(F<int>::f() == ^^F<int>::f);
