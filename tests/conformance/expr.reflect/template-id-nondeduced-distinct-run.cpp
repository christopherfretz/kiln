// [temp.over.link]/5, [temp.deduct.type]/5, [expr.reflect] P2996R13;
// spec/abi.md §8.14 (WP-1260): function templates whose signatures differ
// only in a dependent reflected alias template-id with a parameter in a
// non-deduced context are distinct declarations, and g++-16's spelling,
// which kiln's default -fkiln-reflection-mangling=gcc now writes, keeps
// them apart: `^^D<T>` and `^^D<T*>`, `^^D<T>` and
// `^^typename RC<T>::type` (an alias template-id and its aliased type),
// `^^D2<T, int>` and `^^D2<T, long>`, `^^N1<T>` and `^^N3<T>` (nested
// aliases), `^^DT<T>` and `^^DT<T*>` (a decltype). Each overload of a pair
// is called and returns its own value; the output is compared with GCC
// 16.1's. The pairs GCC folds into one symbol are in
// template-id-nondeduced-collide-run.cpp.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ exit: 0
//@ stdout: 1 2 3 4 5 6 7 8 9 10
#include <cstdio>
#include "Inputs/template-id-nondeduced.h"

template <class T> int a(T, K<^^D<T>>) { return 1; }
template <class T> int a(T, K<^^D<T*>>) { return 2; }
template <class T> int b(T, K<^^D<T>>) { return 3; }
template <class T> int b(T, K<^^typename RC<T>::type>) { return 4; }
template <class T> int c(T, K<^^D2<T, int>>) { return 5; }
template <class T> int c(T, K<^^D2<T, long>>) { return 6; }
template <class T> int d(T, K<^^N1<T>>) { return 7; }
template <class T> int d(T, K<^^N3<T>>) { return 8; }
template <class T> int e(T, K<^^DT<T>>) { return 9; }
template <class T> int e(T, K<^^DT<T*>>) { return 10; }

int main() {
  std::printf("%d %d %d %d %d %d %d %d %d %d\n", a(1, K<^^D<int>>{}),
              a(1, K<^^D<int*>>{}), b(1, K<^^D<int>>{}),
              b(1, K<^^RC<int>::type>{}), c(1, K<^^D2<int, int>>{}),
              c(1, K<^^D2<int, long>>{}), d(1, K<^^N1<int>>{}),
              d(1, K<^^N3<int>>{}), e(1, K<^^DT<int>>{}),
              e(1, K<^^DT<int*>>{}));
}
