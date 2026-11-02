// [dcl.init.general]/16.6.2, [dcl.init.list]/3.2, [temp.inst]: in a
// template, `T x(e)` and `T x{e}` with `e` of class `T` (an lvalue, an
// xvalue, a derived class) are redone at instantiation from the written
// argument: the copy or move constructor is called, never the default
// constructor (libc++ condition_variable_any::wait_until's
// `unique_lock<mutex> __internal_lock2(std::move(__internal_lock))`,
// wait_until_token_pred.pass.cpp). Matches clang 19.
//@ expect: run
//@ stdout: 11 101 11 201 11 101 7
extern "C" int printf(const char*, ...);
template <class T> T&& mv(T& t) { return static_cast<T&&>(t); }
struct UL {
  int v = 0;
  UL() {}
  explicit UL(int x) : v(x) {}
  UL(const UL& u) : v(u.v + 10) {}
  UL(UL&& u) noexcept : v(u.v + 100) {}
};
struct D : UL {
  explicit D(int x) : UL(x) {}
};
struct P {
  int v;
  P(int x = 7) : v(x) {}
};
template <class Q> void f(Q) {
  UL a(1);
  UL b(a);
  UL c(mv(a));
  UL d{a};
  UL e{mv(c)};
  D k(1);
  UL g(k);
  UL h{mv(k)};
  P p;
  printf("%d %d %d %d %d %d %d\n", b.v, c.v, d.v, e.v, g.v, h.v, p.v);
}
int main() { f(0); }
