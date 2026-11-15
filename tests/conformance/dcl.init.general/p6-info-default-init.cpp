// [dcl.init.general]/6.3, /5.1, /7.1 (P2996R13): default-initializing a
// `std::meta::info` zero-initializes it, which gives the null reflection,
// and `info` is const-default-constructible, so a `constexpr` or `const`
// one needs no initializer and is a constant. Found running GCC's C++
// testsuite (g++.dg/reflect: `constexpr info r;` was "must be
// initialized by a constant expression").
//@ expect: compile-ok
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using std::meta::info;

constexpr info r;
static_assert(r == info{} && r == info());
constexpr info r2{};
static_assert(r == r2);

consteval bool local() {
  info x;
  info y = x;
  return x == info{} && y == r && !std::meta::is_annotation(x);
}
static_assert(local());

template <info R = info()> struct W {
  static constexpr bool null = R == info{};
};
static_assert(W<r>::null && W<>::null);

struct S {
  info a{};
  info b = r;
};
constexpr S s;
static_assert(s.a == r && s.b == r);

template <class T> consteval bool dep() {
  T t;
  return t == T{};
}
static_assert(dep<info>());
