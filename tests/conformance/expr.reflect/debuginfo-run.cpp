// [temp.arg.nontype]/2, [basic.fundamental]/17 P2996R13 at `-g` (WP-910):
// every runtime entity specialized on a `std::meta::info` value gets debug
// information (the parameter is a pointer-sized base type named
// `std::meta::info`, with no value), and a consteval-only variable or
// function emits none, so the TU compiles and runs as without `-g`:
// a class template with a virtual function and a static data member, a
// function template, a variable template and a pack of reflections
// (spec/codegen.md §5.1).
//@ expect: run
//@ args: -freflection -I tests/support -g
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: none
//@ stdout: 4 8 1 3 7
#include "kiln_meta.h"
#include <cstdio>
using namespace std::meta;

template <info R> struct X {
  typename[:R:] v{};
  static int count;
  virtual int size() const { return sizeof(v); }
};
template <info R> int X<R>::count = 1;

template <info R> int f() { return static_cast<int>(sizeof(typename[:R:])); }
template <info R> constexpr int width = sizeof(typename[:R:]);
template <info... Rs> struct Many { int n = sizeof...(Rs); };

constexpr info gr = ^^long;
consteval info pick(bool b) { return b ? ^^int : ^^char; }

int main() {
  X<^^int> xi;
  X<pick(false)> xc;
  Many<^^int, ^^char, gr> m;
  constexpr info local = ^^short;
  std::printf("%d %d %d %d %d\n", xi.size(), f<gr>(), X<^^char>::count + xc.size() - 1,
              m.n, width<local> + width<^^int> + 1);
}
