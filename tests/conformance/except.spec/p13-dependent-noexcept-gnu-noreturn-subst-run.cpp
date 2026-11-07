// [except.spec]/13, [temp.inst]/17, [dcl.attr.noreturn] (GNU form):
// a value-dependent `noexcept(expr)` in a function type that also
// carries GNU `noreturn` (a typedef of a pointer, member pointer or
// reference to such a function in a class template) is substituted at
// instantiation. GNU `noreturn` rebuilds the function type from its
// canonical form, whose dependent specification is a profile; kiln
// stopped with "not yet implemented: dependent exception
// specification" (WP-723).
//@ std: c++17
//@ expect: run
//@ strict-diags
//@ stdout: 1 0 1 0
#include <cstdio>
#include <cstdlib>
[[noreturn]] void quit() noexcept { std::exit(0); }
template <bool B> struct S {
  typedef void (*NR)() noexcept(B) __attribute__((noreturn));
  typedef void F() noexcept(B) __attribute__((noreturn));
  static constexpr bool v = noexcept(static_cast<NR>(nullptr)());
};
struct C { [[noreturn]] void m() noexcept { std::exit(0); } };
template <class T> struct M {
  typedef void (C::*PM)() noexcept(sizeof(T) > 1) __attribute__((noreturn));
  typedef void (&R)() noexcept(sizeof(T) > 1) __attribute__((noreturn));
  static constexpr bool v = noexcept((static_cast<C*>(nullptr)->*static_cast<PM>(nullptr))());
};
template <class A, class B> constexpr bool same = false;
template <class A> constexpr bool same<A, A> = true;
typedef void (*NRT)() noexcept __attribute__((noreturn));
typedef void (*NRF)() __attribute__((noreturn));
static_assert(same<S<true>::NR, NRT> && same<S<false>::NR, NRF>);
static_assert(!same<S<true>::NR, void (*)() noexcept>);
static_assert(S<true>::v && !S<false>::v);
static_assert(M<int>::v && !M<char>::v);
static_assert(noexcept(static_cast<typename S<true>::F*>(nullptr)()));
int main() {
  std::printf("%d %d %d %d\n", S<true>::v, S<false>::v, M<int>::v, M<char>::v);
  std::fflush(stdout);
  if (sizeof(int) > 64) quit();
  return 0;
}
