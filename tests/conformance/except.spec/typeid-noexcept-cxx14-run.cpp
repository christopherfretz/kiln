// [except.spec]/1, [expr.typeid] (C++14, pre-P0012): before C++17 the
// exception specification is not part of the function type, so
// `typeid` of a `noexcept` function, function pointer or member function
// pointer names the type without it (`PFvvE`, not `PDoFvvE`) and equals
// the potentially-throwing type's, as clang's does. kiln kept it (WP-751).
//@ expect: run
//@ std: c++14
//@ stdout: PFvvE
//@ stdout: M1SFvvE
//@ stdout: FvvE
//@ stdout: 1WIPFvvEE
//@ stdout: same
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
