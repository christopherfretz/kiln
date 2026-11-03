// [temp.type]/2, [temp.arg.nontype]/2, [basic.link] P2996R13: runtime
// entities specialized on a std::meta::info template argument are emitted
// once per reflection. A function template, a variable template and a
// class template with a static data member and a virtual function each
// get one definition per distinct reflection: ^^int, the alias ^^I and the
// alias ^^J are three specializations, while two uses of ^^I share one
// (one symbol, one function-local static). The bodies consult the
// reflection only through consteval queries (type_of, name_of), never a
// splice; the specializations link within the TU (spec/reflection.md §8.3).
//@ expect: run
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ stdout: f 3 1 3 2 | v 4 8 4 | s 1 2 0 | w 11 12 15 | x 2
#include "kiln_meta.h"
#include <cstdio>
using namespace std::meta;

using I = int;
using J = int;
struct P { int x; double y; };
int gvar;

// A function template: the name length is computed at compile time.
template <info R> int len() {
  constexpr const char* n = name_of(R);
  int k = 0;
  while (n[k]) ++k;
  return k;
}
template <info R> int bump() { static int n = 0; return ++n; }

// A variable template: the size of the reflected entity's type.
template <info R> constexpr int size_v = sizeof(int) * (type_of(R) == ^^int ? 1 : 2);
template <info R> int var = 10;

// A class template with a static member and a virtual function.
template <info R> struct S {
  static int count;
  virtual int get() const {
    constexpr bool dbl = type_of(R) == ^^double;
    return dbl ? 2 : 1;
  }
  virtual ~S() = default;
};
template <info R> int S<R>::count = 0;
template <info R> struct D : S<R> {
  int get() const override { return 10 + S<R>::get(); }
};

// Two reflection arguments: the second literal reuses the info type
// through the substitution table.
template <info A, info B> int two() {
  constexpr bool same = A == B;
  return same ? 1 : 2;
}

int main() {
  // len<^^I> and len<^^J> are distinct functions with equal results.
  bump<^^I>();
  bump<^^I>();
  bump<^^J>();
  std::printf("f %d %d %d %d", len<^^int>(), len<^^I>(), bump<^^I>(), bump<^^J>());
  std::printf(" | v %d %d %d", size_v<^^P::x>, size_v<^^P::y>, size_v<^^gvar>);
  S<^^P::x>::count++;
  S<^^P::y>::count += 2;
  std::printf(" | s %d %d %d", S<^^P::x>::count, S<^^P::y>::count, S<^^int>::count);
  D<^^P::x> dx;
  D<^^P::y> dy;
  S<^^P::x>& rx = dx;
  S<^^P::y>& ry = dy;
  var<^^I> += 1;
  var<^^J> += 2;
  std::printf(" | w %d %d %d", rx.get(), ry.get(), var<^^I> + var<^^J> - var<^^int> + 2);
  std::printf(" | x %d\n", two<^^I, ^^J>() * two<^^int, ^^int>());
  // The aliases name distinct specializations: distinct addresses.
  return (&bump<^^I> == &bump<^^J> || &bump<^^I> == &bump<^^int>) ? 1 : 0;
}
