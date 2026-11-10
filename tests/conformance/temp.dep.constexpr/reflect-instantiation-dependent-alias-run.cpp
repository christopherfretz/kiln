// [temp.dep.constexpr], [temp.alias]/2, [temp.deduct.call]/1,
// [expr.reflect] P2996R13; spec/reflection.md §8.3 (WP-1260): `^^ND<T>`,
// with `template <class T> using ND = int;`, reflects the alias
// specialization `ND<T>` of the template's parameter. The type is not
// dependent (it is `int`) but it is instantiation-dependent, so the
// reflect-expression is value-dependent and substitution rebuilds it.
// Before WP-1260 kiln took its value at definition time, so the parameter
// type `K<^^ND<T>>` was never substituted and `p(1, K<^^ND<int>>{})`
// found no viable candidate ("no known conversion from 'K<^^ND<int>>' to
// 'K<^^ND<type-parameter-0-0>>'"), and `r<int>` was not `^^ND<int>`.
// (Kiln's `^^ND<int>` and `^^ND<long>` are distinct reflections, g++-16's
// compare equal, so this test checks only what both agree on; the
// distinct overloads are in expr.reflect/template-id-nondeduced-collide-run.cpp.)
// The output is compared with GCC 16.1's.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ exit: 0
//@ stdout: 1 1 1 1
#include <cstdio>
using info = decltype(^^int);
template <info R> struct K {};
template <class T> using ND = int;

template <class T> int p(T, K<^^ND<T>>) { return 1; }
template <class T> int same() {
  constexpr bool b = ^^ND<T> == ^^ND<int>;
  return b;
}
template <class T> constexpr info r = ^^ND<T>;

static_assert(r<int> == (^^ND<int>));

int main() {
  std::printf("%d %d %d %d\n", p(1, K<^^ND<int>>{}), p<long>(1, K<^^ND<long>>{}),
              p(2L, K<r<long>>{}), same<int>());
}
