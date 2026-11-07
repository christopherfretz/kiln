// [except.spec]/13, [temp.deduct.call]: a function template
// specialization's exception specification is part of the type deduced
// from `&h<true>` in a template's body as outside one, so the deduced
// `F` is `void (*)() noexcept` (kiln deduced `void (*)()` inside
// `Outer<T>::run`); `__PRETTY_FUNCTION__` prints it as clang 19 and 22 do.
//@ expect: run
//@ std: c++17
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)() noexcept]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)()]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (S<true>::*)() noexcept]
//@ stdout: static void Outer<int>::k(F) [T = int, F = void (*)(int)]
//@ stdout: static void Outer<char>::k(F) [T = char, F = void (*)(char) noexcept]
#include <cstdio>
template <bool B> struct S { void m() noexcept(B) {} };
template <bool B> void h() noexcept(B) {}
template <class T> void h2(T) noexcept(sizeof(T) == 1) {}
template <class T> struct Outer {
  template <class F> static void k(F) { puts(__PRETTY_FUNCTION__); }
  static void run() { k(&h<true>); k(&h<false>); k(&S<true>::m); k(&h2<T>); }
};
int main() {
  Outer<int>::run();
  Outer<char>::k(&h2<char>);
}
