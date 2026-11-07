// [dcl.fct.def.general]/8, [except.spec]/13: a template argument whose
// type is a specialization's function or member function type with an
// instantiated `noexcept(B)` prints in `__PRETTY_FUNCTION__`'s
// ` [T = ...]` list with a trailing `noexcept` when B is true and nothing
// when it is false, deduced (`t(&S<true>::m)`), explicit (`u<...>`) or
// deduced inside a class template's member. Matches clang 19 and 22 byte
// for byte (WP-753).
//@ expect: run
//@ std: c++17
//@ stdout: void t(T) [T = void (S<false>::*)()]
//@ stdout: void t(T) [T = void (S<true>::*)() noexcept]
//@ stdout: void t(T) [T = void (*)()]
//@ stdout: void t(T) [T = void (*)() noexcept]
//@ stdout: void t(T) [T = void (S<true>::*)(int) noexcept]
//@ stdout: void t(T) [T = void (S<false>::*)(int)]
//@ stdout: void t(T) [T = void (*)(char)]
//@ stdout: void t(T) [T = void (*)(int) noexcept]
//@ stdout: void u() [T = void (*)()]
//@ stdout: void u() [T = void (*)(char)]
//@ stdout: void u() [T = void (int) noexcept]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)() noexcept]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)()]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (S<true>::*)() noexcept]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)(int) noexcept]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)() noexcept]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)()]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (S<true>::*)() noexcept]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)(char)]
#include <cstdio>
template <bool B> struct S { void m() noexcept(B) {} static void s() noexcept(B) {} template <class U> void tm(U) noexcept(B) {} };
template <bool B> void h() noexcept(B) {}
template <class T> void h2(T) noexcept(sizeof(T) > 1) {}
template <class T> void t(T) { puts(__PRETTY_FUNCTION__); }
template <class T> void u() { puts(__PRETTY_FUNCTION__); }
template <class T> struct Outer { template <class F> static void k(F) { puts(__PRETTY_FUNCTION__); } static void run() { k(&h<true>); k(&h<false>); k(&S<true>::m); k(&h2<T>); } };
int main() {
  t(&S<false>::m); t(&S<true>::m); t(&S<false>::s); t(&S<true>::s); t(&S<true>::tm<int>); t(&S<false>::tm<int>);
  t(&h2<char>); t(&h2<int>);
  u<decltype(&S<false>::s)>(); u<decltype(&h2<char>)>(); u<decltype(h2<int>)>();
  Outer<int>::run(); Outer<char>::run();
}
