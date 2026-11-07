// [except.spec]/1, Itanium ABI <exception-spec>: before C++17 (pre-P0012)
// an exception specification is not part of the function type and is
// not mangled: a function pointer, member function pointer, reference or
// template argument type, deduced, explicit or `decltype(&f)`, mangles
// without `Do` (`void (*)() noexcept` is `PFvvE`), and `t(&f)` and
// `t(&g)` are one specialization. kiln mangled `Do` at c++11 (WP-751).
//@ expect: ir
//@ min-oracle: 22
//@ std: c++11
void f() noexcept {}
void g() {}
struct S {
  void m() noexcept {}
  void n() const {}
};
template <class T> void t(T) {}
template <void (*P)() noexcept> void v() {}
template <class T> struct W {};
void p1(void (*)() noexcept) {}
void p2(void (S::*)() noexcept) {}
void p3(void (&)() noexcept) {}
void p4(W<void() noexcept>) {}
void p5(W<decltype(&f)>) {}
void (*fp)() noexcept = f;
void (S::*mp)() noexcept = &S::m;
decltype(&f) dp = f;
void use() {
  t(&f);
  t(&g);
  t(&S::m);
  t(f);
  v<f>();
  fp();
  (S().*mp)();
}
