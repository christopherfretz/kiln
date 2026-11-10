// [temp.over.link]/5, [expr.reflect] P2996R13; spec/abi.md §8.14
// (WP-1250): function templates whose signatures differ only in a
// dependent reflected template-id are distinct declarations, and where
// GCC 16.1's spelling keeps them apart kiln's default
// -fkiln-reflection-mangling=gcc symbols do too: `^^AT<T>` and
// `^^TT<T*>` (an alias template-id and its aliased type), `^^AT<T>` and
// `^^AT<T*>`, `^^g<T>` and `^^h<T>`, `^^vt<T>` and `^^nv<T>`,
// `^^Arr<T, 3>` and `^^Arr<T, 4>` (`TT<T[N]>`: both parameters deduced). Each
// overload of a pair is called and returns its own value; the output is
// compared with GCC 16.1's. The pairs GCC folds into one symbol are in
// template-id-signature-collide-run.cpp.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ exit: 0
//@ stdout: 1 2 3 4 5 6 7 8 9 10
#include <cstdio>
#include "Inputs/template-id-signature.h"

template <class T> int a(T, K<^^AT<T>>) { return 1; }
template <class T> int a(T, K<^^TT<T*>>) { return 2; }
template <class T> int b(T, K<^^AT<T>>) { return 3; }
template <class T> int b(T, K<^^AT<T*>>) { return 4; }
template <class T> int c(T, K<^^g<T>>) { return 5; }
template <class T> int c(T, K<^^h<T>>) { return 6; }
template <class T> int d(T, K<^^vt<T>>) { return 7; }
template <class T> int d(T, K<^^nv<T>>) { return 8; }
template <class T> int e(T, K<^^Arr<T, 3>>) { return 9; }
template <class T> int e(T, K<^^Arr<T, 4>>) { return 10; }

int main() {
  std::printf("%d %d %d %d %d %d %d %d %d %d\n", a(1, K<^^AT<int>>{}),
              a(1, K<^^TT<int*>>{}), b(1, K<^^AT<int>>{}),
              b(1, K<^^AT<int*>>{}), c(1, K<^^g<int>>{}), c(1, K<^^h<int>>{}),
              d(1, K<^^vt<int>>{}), d(1, K<^^nv<int>>{}),
              e(1, K<^^Arr<int, 3>>{}), e(1, K<^^Arr<int, 4>>{}));
}
