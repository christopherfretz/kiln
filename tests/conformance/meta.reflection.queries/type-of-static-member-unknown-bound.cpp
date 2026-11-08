// [meta.reflection.queries] type_of, [temp.inst]/4: a static data member
// of a class template declared with an array of unknown bound and defined
// out of class with an initializer gets its bound from that definition;
// type_of of a specialization's member instantiates the definition and
// sees the complete type. Before the member's definition is seen the
// type stays incomplete. GCC 16.1 agrees (WP-796).
//@ expect: run
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ stdout: 3 3 4
#include <meta>
#include <cstdio>

template <class T> struct C {
  static T v[];
  static const int w;
};
template <class T> T C<T>::v[] = {1, 2, 3};
template <class T> const int C<T>::w = 4;

static_assert(std::meta::type_of(^^C<int>::v) == ^^int[3]);
static_assert(std::meta::type_of(^^C<int>::w) == ^^const int);
static_assert(std::meta::type_of(^^C<long>::v) == ^^long[3]);

template <class T> struct D { static T u[]; };
static_assert(std::meta::type_of(^^D<int>::u) == ^^int[]);
template <class T> T D<T>::u[] = {5, 6, 7, 8};
static_assert(std::meta::type_of(^^D<short>::u) == ^^short[4]);

int main() {
  std::printf("%d %d %d\n", (int)std::meta::extent(std::meta::type_of(^^C<char>::v)),
              C<char>::v[2], (int)sizeof(D<char>::u));
}
