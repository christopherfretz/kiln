// [except.spec]/1 (C++14), [temp.variadic]/5: the type-id-list of a dynamic
// exception specification may hold pack expansions, expanded when the
// template is instantiated. An empty expansion is `throw()`, so the
// function is non-throwing ([except.spec]/12 as of C++14). C++17 removed
// the form; see p1-dynamic-pack-expansion-cxx17.cpp (WP-722).
//@ std: c++14
//@ expect: run
//@ strict-diags
//@ stdout: 28 1 0 0 3
#include <cstdio>
struct A { int v; };
struct B { int w; };
template <class... Ts> void f(int k) throw(Ts...) {
  if (k == 1) throw A{7};
  if (k == 2) throw B{8};
}
template <class... Ts> struct S {
  static int g(int k) throw(Ts..., long) { if (k) throw 5L; return 3; }
  void (*fp)(int) throw(Ts...) = &f<Ts...>;
};
int main() {
  int r = 0;
  try { f<A, B>(1); } catch (A a) { r += a.v; }
  try { f<A, B>(2); } catch (B b) { r += b.w; }
  try { S<A>::g(1); } catch (long l) { r += int(l); }
  S<A, B> s;
  try { s.fp(2); } catch (B b) { r += b.w; }
  std::printf("%d %d %d %d %d\n", r, int(noexcept(f<>(0))), int(noexcept(f<A>(0))),
              int(noexcept(S<>::g(0))), S<>::g(0));
}
