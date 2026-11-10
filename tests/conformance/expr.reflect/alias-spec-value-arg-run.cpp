// [temp.alias]/2, [temp.type]/1-2, [expr.reflect]/6 P2996R13: an alias
// template specialization's reflection is keyed on its converted
// arguments, so `^^Arr<char, 3>`, `^^Arr<char, 1 + 2>` and
// `^^Arr<char, (short)3>` are one std::meta::info value and a function
// template specialized on it is one specialization: the three calls share
// one function-local static. The same holds for an enumerator and a `bool`
// argument (`EB<E::b, true>` and `EB<static_cast<E>(1), 1>`), while
// `Arr<char, 4>` and the aliased type `char[3]` are their own
// specializations (spec/sema-templates.md §4.10, WP-1168).
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ stdout: arr 1 2 3 | four 1 | type 1 | eb 1 2 | ea 1
#include "kiln_meta.h"
#include <cstdio>

template <class T, int N> using Arr = T[N];
enum class E { a, b };
template <E e, bool B> using EB = int[int(e) + 2 * B + 1];

template <std::meta::info R> int bump() {
  static int n = 0;
  return ++n;
}

int main() {
  int a = bump<^^Arr<char, 3>>();
  int b = bump<^^Arr<char, 1 + 2>>();
  int c = bump<^^Arr<char, (short)3>>();
  int d = bump<^^Arr<char, 4>>();
  int t = bump<^^char[3]>();
  int e = bump<^^EB<E::b, true>>();
  int f = bump<^^EB<static_cast<E>(1), 1>>();
  int g = bump<^^EB<E::a, true>>();
  std::printf("arr %d %d %d | four %d | type %d | eb %d %d | ea %d\n", a, b, c, d, t, e, f, g);
}
