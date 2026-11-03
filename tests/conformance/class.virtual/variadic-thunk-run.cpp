// [class.virtual]: calling a variadic virtual function through a base
// that needs a this adjustment (a thunk) passes the variadic arguments on
// unchanged, for scalar, aggregate and void results and through a
// virtual base.
//@ expect: run
//@ stdout: 6 3.5 9 x 12 0 1 2 30
#include <cstdarg>
#include <cstdio>
struct Big { long a, b, c, d; };
struct A { virtual int f(int, ...) { return -1; } int a = 0; };
struct B {
  virtual int f(int, ...) { return -1; }
  virtual Big big(int, ...) { return {}; }
  virtual void v(const char *, ...) {}
  int b = 0;
};
struct C : A, B {
  int f(int n, ...) override {
    va_list ap;
    va_start(ap, n);
    int s = 0;
    for (int i = 0; i < n; ++i) s += va_arg(ap, int);
    va_end(ap);
    return s;
  }
  Big big(int n, ...) override {
    va_list ap;
    va_start(ap, n);
    Big r{va_arg(ap, long), va_arg(ap, long), va_arg(ap, long), 0};
    va_end(ap);
    return r;
  }
  void v(const char *fmt, ...) override {
    va_list ap;
    va_start(ap, fmt);
    std::vprintf(fmt, ap);
    va_end(ap);
  }
};
struct V { virtual long w(int, ...) { return -1; } long x = 0; };
struct D : virtual V {
  long w(int n, ...) override {
    va_list ap;
    va_start(ap, n);
    long s = 0;
    for (int i = 0; i < n; ++i) s += va_arg(ap, long);
    va_end(ap);
    return s;
  }
};
int main() {
  C c;
  B *b = &c;
  int s = b->f(3, 1, 2, 3);
  b->v("%d %.1f ", s, 3.5);
  b->v("%d %c ", b->f(2, 4, 5), 'x');
  Big g = b->big(3, 12L, 0L, 1L);
  std::printf("%ld %ld %ld ", g.a, g.b, g.c);
  D d;
  V *v = &d;
  std::printf("%ld %ld\n", 2L, v->w(3, 10L, 10L, 10L));
}
