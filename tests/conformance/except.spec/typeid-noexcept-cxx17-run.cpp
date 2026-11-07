// [except.spec]/1, [expr.typeid]: since C++17 (P0012) the exception
// specification is part of the function type, so `typeid` of a
// `noexcept` function, function pointer or member function pointer names
// it (`PDoFvvE`) and differs from the potentially-throwing type's, as
// clang's does (WP-751).
//@ expect: run
//@ std: c++17
//@ stdout: PDoFvvE
//@ stdout: M1SDoFvvE
//@ stdout: DoFvvE
//@ stdout: 1WIPDoFvvEE
//@ stdout: distinct
#include <cstdio>
#include <typeinfo>
void f() noexcept {}
struct S { void m() noexcept {} };
template <class T> struct W {};
int main() {
  puts(typeid(&f).name());
  puts(typeid(&S::m).name());
  puts(typeid(f).name());
  puts(typeid(W<decltype(&f)>).name());
  puts(typeid(void (*)() noexcept) == typeid(void (*)()) ? "same" : "distinct");
}
