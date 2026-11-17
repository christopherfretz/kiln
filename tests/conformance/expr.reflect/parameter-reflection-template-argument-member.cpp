// [temp.arg.nontype], [expr.const] (P2996R13): the sibling of
// parameter-reflection-template-argument.cpp in a member function
// template of a class template, in a pack expansion of template
// arguments, and as a default template argument: each argument is one
// constant expression in the instantiation. WP-1613.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
#include <meta>
template <std::meta::info... I> int use() { return sizeof...(I); }
template <class T> struct X {
  template <int N> int f(T x, int y) {
    return use<std::meta::parameters_of(^^X<T>::f<N>)[N]>() +
           use<std::meta::parameters_of(^^X<T>::f<N>)[0],
               std::meta::parameters_of(^^X<T>::f<N>)[1]>();
  }
};
template int X<long>::f<1>(long, int);
template <int N, std::meta::info D = std::meta::members_of(^^std::meta, std::meta::access_context::unchecked())[N]>
int dflt() { return 0; }
template <int N> int g() { return dflt<N>(); }
template int g<0>();
