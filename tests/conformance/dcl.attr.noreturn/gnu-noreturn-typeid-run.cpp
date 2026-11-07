// GNU `noreturn` adds to the function type and keeps its other bits, so
// the type's mangled name (`typeid(...).name()`, [expr.typeid],
// Itanium C++ ABI 5.1.5.3) still carries the exception specification
// (`Do`), the member function's cv- and ref-qualifiers and the ellipsis,
// for a plain declaration, a typedef'd one and an instantiated member
// alike. kiln lost the bits of a typedef'd type with a dependent
// `noexcept(B)` and the `noreturn` of an instantiated member.
//@ expect: run
//@ std: c++17
//@ stdout: PDoFvvE
//@ stdout: PDoFvizE
//@ stdout: M1AKDoFvvRE
//@ stdout: M1AVFvizOE
//@ stdout: PDoFvvE
//@ stdout: PDoFvvE
//@ stdout: PFvvE
//@ stdout: PDoFvvE
//@ stdout: PFvvE
#include <cstdio>
#include <typeinfo>
struct A {
  void m() const & noexcept __attribute__((noreturn));
  void n(int, ...) volatile && __attribute__((noreturn));
};
static void die() noexcept __attribute__((noreturn));
static void die() noexcept { __builtin_trap(); }
void var(int, ...) noexcept __attribute__((noreturn));
typedef void F() noexcept;
F d1 __attribute__((noreturn));
template <bool B> struct S {
  typedef void G() noexcept(B);
  static G e __attribute__((noreturn));
  static void f() noexcept(B) __attribute__((noreturn));
};
#define SHOW(x) std::puts(typeid(x).name())
int main() {
  SHOW(&die);
  SHOW(&var);
  SHOW(&A::m);
  SHOW(&A::n);
  SHOW(&d1);
  SHOW(&S<true>::e);
  SHOW(&S<false>::e);
  SHOW(&S<true>::f);
  SHOW(&S<false>::f);
}
