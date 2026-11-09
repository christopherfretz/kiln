// [dcl.fct.default]/3: default arguments in function declarations of every
// form (parenthesized declarator-id, trailing return type, function
// returning a pointer, members, constructors, friends, block-scope and
// init-statement declarations) and lambda declarators; an
// exception-declaration's nested parameters are not checked (as clang).
// Invalid pair: nonfunction-declarator-default-arg.cpp (WP-1015).
//@ std: c++20
//@ expect: compile-ok
void *f(int = 1);
int &g(int = 1);
int *const *h(int a = 1, int b = 2);
auto t(int = 3) -> void (*)(int);
void (*ret(int = 4))(int);
template <class T> struct A { A(T = T()); template <class U> void m(U = U()); };
struct S {
  S(int = 1);
  void f(int = 2) const &;
  static void sf(int = 3) noexcept;
  bool operator()(int = 4);
  friend void fd(int = 5) {}
  void (g)(int = 6);
};
namespace N { void nf(int = 1); }
void N::nf(int) {}
void l() {
  extern void lf(int = 1);
  auto la = [](int a = 1, int (*p)(int) = nullptr) { return a + (p ? 1 : 0); };
  la();
  if (void k(int = 1); true) {}
  for (void k2(int = 2);;) { break; }
  try {} catch (void (*e)(int = 4)) {}
}
template <class T> auto vt(T x = T()) -> decltype(x);
