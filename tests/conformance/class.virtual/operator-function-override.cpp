// [class.virtual]/2, [over.oper]/1: an operator function overrides a
// virtual operator function of a base with the same parameter-type-list
// like any other member function, so the derived class is not abstract
// and calls dispatch to it (at run time and in constant evaluation).
// libc++ function.h:361 `__func::operator()` overriding
// `__base::operator()`; future:1497 `__packaged_task_func` (WP-276).
//@ expect: run
//@ stdout: 3 7 11 4
#include <cstdio>
struct B {
  virtual int operator()(int) = 0;
  virtual int operator+(int) const = 0;
  virtual ~B() = default;
};
struct D : B {
  int operator()(int x) override { return x + 1; }
  int operator+(int x) const override { return x + 5; }
};
template <class R, class... A> struct base {
  virtual R operator()(A &&...) = 0;
  virtual ~base() = default;
};
template <class F, class R, class... A> struct func : base<R, A...> {
  F f;
  constexpr func(F g) : f(g) {}
  constexpr R operator()(A &&...a) override { return f(a...); }
};
constexpr int seven() { return 7; }
constexpr int via_base() {
  func<int (*)(), int> g(seven);
  base<int> &b = g;
  return b();
}
static_assert(via_base() == 7);
int main() {
  D d;
  B &b = d;
  base<int> *p = new func<int (*)(), int>(seven);
  auto inc = [](int x) { return x + 2; };
  base<int, int> *q = new func<decltype(inc), int, int>(inc);
  std::printf("%d %d %d %d\n", b(2), (*p)(), b + 6, (*q)(2));
  delete p;
  delete q;
}
