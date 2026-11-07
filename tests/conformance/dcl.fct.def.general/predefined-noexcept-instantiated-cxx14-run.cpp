// [dcl.fct.def.general]/8, [except.spec] (C++14, pre-P0012): before
// C++17 the exception specification is not part of the function type, so
// a deduced or explicit template argument of function, function pointer
// or member pointer type prints without `noexcept`, whatever the
// specialization's `noexcept(B)`. Matches clang 19 and 22 byte for byte
// (WP-751, WP-753).
//@ expect: run
//@ std: c++14
//@ stdout: void t(T) [T = void (S<false>::*)()]
//@ stdout: void t(T) [T = void (S<true>::*)()]
//@ stdout: void t(T) [T = void (*)()]
//@ stdout: void t(T) [T = void (*)()]
//@ stdout: void t(T) [T = void (S<true>::*)(int)]
//@ stdout: void t(T) [T = void (S<false>::*)(int)]
//@ stdout: void t(T) [T = void (*)(char)]
//@ stdout: void t(T) [T = void (*)(int)]
//@ stdout: void u() [T = void (*)()]
//@ stdout: void u() [T = void (*)(char)]
//@ stdout: void u() [T = void (int)]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)()]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)()]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (S<true>::*)()]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)(int)]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)()]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)()]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (S<true>::*)()]
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
