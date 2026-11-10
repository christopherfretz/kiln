// [temp.over.link]/5, [expr.reflect] P2996R13; spec/abi.md §8.14 "Known
// divergences from GCC 16.1" (WP-1250): function templates whose
// signatures differ only in the template arguments (or the qualifier) of
// a dependent reflected template-id are distinct declarations, so their
// specializations need distinct symbols. GCC 16.1 writes a function
// template-id as `ft <name>` without its arguments (`^^g<T>` and
// `^^g<T*>`, `^^ns::operator== <T>` and `^^ns::operator== <T*>`), a
// qualified variable template-id without its qualifier (`^^ns::nv<T>` and
// `^^nv<T>`), and an alias template-id by its aliased type's arguments
// even when an alias parameter does not appear there (`^^F<T, int>` and
// `^^F<T, long>`): each pair gets one symbol and g++-16 fails to assemble
// ("symbol ... is already defined"); so do the non-dependent
// `^^F<int, int>` and `^^F<int, long>`, and `^^D<int>` and
// `^^D<const int>` (`typename RC<T>::type`, a remove_const: both `ta1D_i`
// in GCC), as template arguments of `e`, each
// of whose specializations counts its own calls. Kiln keeps the arguments and the
// qualifier in both -fkiln-reflection-mangling modes, so each overload
// is its own function and returns its own value.
//@ expect: run
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 gives each pair one symbol (an ABI bug, spec/abi.md §8.14)
//@ stdout: 1 2 3 4 5 6 7 8 9 9 9 9
#include <cstdio>
#include "Inputs/template-id-signature.h"

template <class T> int a(T, K<^^g<T>>) { return 1; }
template <class T> int a(T, K<^^g<T*>>) { return 2; }
template <class T> int b(T, K<^^ns::operator== <T>>) { return 3; }
template <class T> int b(T, K<^^ns::operator== <T*>>) { return 4; }
template <class T> int c(T, K<^^ns::nv<T>>) { return 5; }
template <class T> int c(T, K<^^nv<T>>) { return 6; }
template <class T> int d(T, K<^^F<T, int>>) { return 7; }
template <class T> int d(T, K<^^F<T, long>>) { return 8; }
template <info R> int e() { static int n = 8; return ++n; }

int main() {
  std::printf("%d %d %d %d %d %d %d %d %d %d %d %d\n", a(1, K<^^g<int>>{}),
              a(1, K<^^g<int*>>{}), b(1, K<^^ns::operator== <int>>{}),
              b(1, K<^^ns::operator== <int*>>{}), c(1, K<^^ns::nv<int>>{}),
              c(1, K<^^nv<int>>{}), d(1, K<^^F<int, int>>{}),
              d(1, K<^^F<int, long>>{}), e<^^F<int, int>>(),
              e<^^F<int, long>>(), e<^^D<int>>(), e<^^D<const int>>());
}
