// [expr.reflect]/6, [stmt.expand]: `^^A` of a type alias represents the
// alias. GCC 16.1 instantiates the body of an expansion statement as a
// template definition and, finding no local specialization of an alias
// declared outside every template, drops one level of it (gcc/cp/pt.cc
// tsubst, "We don't have an instantiation yet, so drop the typedef"): in
// a `template for` body `^^A` is `^^int`. kiln matches GCC 16.1, the
// reflection reference (spec/reflection.md §2.3). A binding generator
// relies on it: it tells a `std::meta::info` template argument from a
// class-type one by `remove_cv(type_of(a)) == ^^std::meta::info` inside
// `template for`. Aliases declared in the body, and every alias outside
// such a body, are kept.
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif

using A = int;
using B = A;
struct S {
  using M = long;
};
static_assert(^^A != ^^int);

template <int... Is> struct L {
  static constexpr int v[] = {Is...};
};

// The element's value makes each comparison dependent, so it is formed
// again for each element, as the binding generator's are.
consteval int plain() {
  int r = 0;
  template for (constexpr std::meta::info t : {^^int, ^^char}) {
    if (t == ^^A) r += 1;            // dropped: ^^A is ^^int here
    if (t == ^^B) r += 10;           // one level: ^^B is ^^A, not ^^int
    if (t != ^^S::M) r += 100;       // a member of a non-template class
  }
  return r;
}
static_assert(plain() == 201);

template <class T> consteval int in_template() {
  int r = 0;
  template for (constexpr int i : T::v) {
    using Local = char;
    if constexpr (^^A == (i > 0 ? ^^int : ^^void)) r += i;
    if constexpr (^^Local != (i > 0 ? ^^char : ^^void)) r += 10 * i;  // kept
  }
  return r;
}
static_assert(in_template<L<1, 2>>() == 33);

consteval bool outside() { return ^^A == ^^int; }
static_assert(!outside());

int main() {}
