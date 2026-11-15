// [meta.reflection]: a metafunction is a function; its address may be
// taken and it may be called through the pointer (or reference) in a
// constant expression, with the result a direct call gives: a scalar, an
// info, or a class (string_view). Found running GCC's C++ testsuite.
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

consteval decltype(sizeof 0) invoke(decltype(sizeof 0) (*fp)(info), info r) { return fp(r); }
consteval info apply(info (&fr)(info), info r) { return fr(r); }

template <class F>
consteval auto call(F f, info r) { return f(r); }

struct S { int m; };

static_assert(invoke(size_of, ^^int) == sizeof(int));
static_assert(invoke(&alignment_of, ^^double) == alignof(double));
static_assert(apply(type_of, ^^S::m) == ^^int);
static_assert(call(has_identifier, ^^S::m));
static_assert(call(identifier_of, ^^S::m) == "m");

consteval int count(info r) {
  info (*fs[2])(info) = {type_of, nullptr};
  fs[1] = &type_of;
  int k = 0;
  for (auto f : fs)
    k += f && f(r) == ^^int;
  return k;
}
static_assert(count(^^S::m) == 2);
