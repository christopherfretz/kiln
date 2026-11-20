// [expr.cond]/4.2: an xvalue operand converts to "rvalue reference to T2"
// of the other xvalue's type when it binds directly, so `b ? f() : g()`
// with `int&& f()` and `const int&& g()` is an xvalue of type `const
// int`; with `volatile int&&` neither binds and the result is a prvalue
// `int` ([expr.cond]/7).
//@ expect: run
//@ stdout: 2 1 1
#include <cstdio>
template<class T, class U> struct ST { static constexpr int v = 0; };
template<class T> struct ST<T, T> { static constexpr int v = 1; };
int x = 1;
const int cx = 2;
int&& f() { return static_cast<int&&>(x); }
const int&& g() { return static_cast<const int&&>(cx); }
volatile int&& h();
static_assert(ST<decltype(true ? f() : g()), const int&&>::v == 1);
static_assert(ST<decltype(true ? g() : f()), const int&&>::v == 1);
static_assert(ST<decltype(true ? g() : h()), int>::v == 1);
int main(int argc, char**) {
  const int&& r = argc > 5 ? f() : g();
  const int&& s = argc > 5 ? g() : f();
  std::printf("%d %d %d\n", r, s, &r == &cx && &s == &x);
}
