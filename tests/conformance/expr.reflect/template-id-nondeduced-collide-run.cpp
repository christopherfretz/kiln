// [temp.over.link]/5, [temp.deduct.type]/5, [expr.reflect] P2996R13;
// spec/abi.md §8.14 "Known divergences from GCC 16.1" (WP-1260): function
// templates whose signatures differ only in the arguments of a dependent
// reflected alias template-id are distinct declarations, so their
// specializations need distinct symbols. GCC 16.1 writes the alias by
// its aliased type even where that loses an argument: `^^ND<T>` and
// `^^ND<T*>` (`ND = int`: both `ta2ND_i`), `^^DL<T, int>` and
// `^^DL<T, const int>` (the second parameter's qualifier resolves to
// `int` for both), `^^DP<int, T>` and `^^DP<const int, T>`: each pair
// gets one symbol and g++-16 fails to assemble ("symbol ... is already
// defined"). Kiln keeps the alias's arguments for these in both
// -fkiln-reflection-mangling modes, so each overload is its own function
// and returns its own value.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 gives each pair one symbol (an ABI bug, spec/abi.md §8.14)
//@ stdout: 1 2 3 4 5 6
#include <cstdio>
#include "Inputs/template-id-nondeduced.h"

template <class T> int a(T, K<^^ND<T>>) { return 1; }
template <class T> int a(T, K<^^ND<T*>>) { return 2; }
template <class T> int b(T, K<^^DL<T, int>>) { return 3; }
template <class T> int b(T, K<^^DL<T, const int>>) { return 4; }
template <class T> int c(T, K<^^DP<int, T>>) { return 5; }
template <class T> int c(T, K<^^DP<const int, T>>) { return 6; }

int main() {
  std::printf("%d %d %d %d %d %d\n", a(1, K<^^ND<int>>{}),
              a(1, K<^^ND<int*>>{}), b(1, K<^^DL<int, int>>{}),
              b(1, K<^^DL<int, const int>>{}), c(1, K<^^DP<int, int>>{}),
              c(1, K<^^DP<const int, int>>{}));
}
