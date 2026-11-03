// [temp.arg.nontype]/2, [temp.type]/2 P2996R13: a reflection is a
// structural value; two `template <info R>` arguments are
// template-argument-equivalent iff the reflections compare equal
// ([expr.eq]/6), so every operand kind names its own specialization and
// equal reflections share one (one symbol, one function-local static).
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ stdout: ok 2 2 2
#include "kiln_meta.h"
#include <cstdio>
using namespace std::meta;

using I = int;
namespace N {}
namespace NA = N;
template <class T> struct Tm {};
int v;
struct S { int m; };
enum E { e0 };

template <info R> int count() { static int n = 0; return ++n; }

constexpr info kinds[] = {
  ^^int, ^^I, ^^N, ^^NA, ^^::, ^^Tm, ^^Tm<int>, ^^v, ^^S::m, ^^e0, ^^E, info{},
};

template <info A, info B> constexpr bool same() { return &count<A> == &count<B>; }

static_assert(same<^^int, ^^int>());
static_assert(same<^^I, ^^I>());
static_assert(same<info{}, info{}>());
static_assert(same<^^Tm<int>, ^^Tm<int>>());
static_assert(!same<^^int, ^^I>());   // the alias is its own entity
static_assert(!same<^^N, ^^NA>());
static_assert(!same<^^N, ^^::>());
static_assert(!same<^^Tm, ^^Tm<int>>());
static_assert(!same<^^v, ^^S::m>());
static_assert(!same<^^e0, ^^E>());
static_assert(!same<^^int, info{}>());

template <int... Is> constexpr bool all_distinct() {
  constexpr int (*ps[])() = {&count<kinds[Is]>...};
  for (int i = 0; i < (int)sizeof...(Is); ++i)
    for (int j = 0; j < i; ++j)
      if (ps[i] == ps[j]) return false;
  return true;
}
static_assert(all_distinct<0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11>());

int main() {
  // Every kind instantiates and runs; equal arguments share the static.
  int sum = count<^^int>() + count<^^I>() + count<^^N>() + count<^^NA>() +
            count<^^::>() + count<^^Tm>() + count<^^Tm<int>>() + count<^^v>() +
            count<^^S::m>() + count<^^e0>() + count<^^E>() + count<info{}>();
  int a = count<^^NA>();
  int b = count<kinds[0]>();   // the same specialization as count<^^int>
  int c = count<kinds[11]>();   // count<info{}>
  std::printf("%s %d %d %d\n", sum == 12 ? "ok" : "bad", a, b, c);
  return 0;
}
