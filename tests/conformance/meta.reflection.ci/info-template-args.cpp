// [temp.arg.nontype], [temp.type]/2 (P2996R13): std::meta::info is a
// structural type, so a reflection is a template argument; explicit
// specializations ([temp.expl.spec]) select on it, a consteval search of a
// constexpr table finds it, and a fold over an info pack compares it.
// Compared with GCC 16.1 (spec/harness.md "GCC oracle"); the exit status
// (idx<^^long> + 1 == 0) is compared too.
//@ expect: run
//@ oracle: gcc
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ stdout: int double namespace N N::S other
//@ stdout: 1 4 -1 3
//@ stdout: 2 N::S
#include <cstdio>
using info = decltype(^^int);
template <info R> struct Name { static constexpr const char* value = "other"; };
template <> struct Name<^^int> { static constexpr const char* value = "int"; };
template <> struct Name<^^double> { static constexpr const char* value = "double"; };
namespace N { struct S {}; }
template <> struct Name<^^N> { static constexpr const char* value = "namespace N"; };
template <> struct Name<^^N::S> { static constexpr const char* value = "N::S"; };

constexpr info table[] = {^^char, ^^int, ^^N::S, ^^double, ^^N};
consteval int index_of(info r) {
  for (int i = 0; i < 5; ++i)
    if (table[i] == r) return i;
  return -1;
}
template <info R> constexpr int idx = index_of(R);
template <info... Rs> consteval int count_matching(info r) {
  int n = 0;
  ((n += (Rs == r)), ...);
  return n;
}
consteval info nth(int i) { return table[i]; }

int main() {
  std::printf("%s %s %s %s %s\n", Name<^^int>::value, Name<^^double>::value, Name<^^N>::value,
              Name<^^N::S>::value, Name<^^char>::value);
  std::printf("%d %d %d %d\n", idx<^^int>, idx<^^N>, idx<^^long>, idx<nth(3)>);
  constexpr int n = count_matching<^^int, ^^char, ^^int, ^^N>(^^int);
  std::printf("%d %s\n", n, Name<nth(2)>::value);
  return idx<^^long> + 1;
}
