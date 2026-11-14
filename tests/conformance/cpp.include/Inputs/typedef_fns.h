// Forms a precompiled header's snapshot carries (WP-1369): functions
// declared through a typedef, an alias template, decltype or __typeof;
// a va_list passed on; bases and member classes of an implicit
// instantiation.
#ifndef TYPEDEF_FNS_H
#define TYPEDEF_FNS_H
#include <stdarg.h>
namespace tf {
typedef int F(int);
typedef int N(int) noexcept;
template <class T> using Un = T(T);
F twice;
int twice(int = 4);
N thrice;
struct Q {
  F inc;
  F dec;
};
inline int Q::inc(int a) { return a + 1; }
template <class T> struct Box {
  Un<T> half;
  static N quad;
};
template <class T> T Box<T>::half(T t) { return t / 2; }
template <class T> int Box<T>::quad(int x) noexcept { return x * 4; }
int base_fn(long);
decltype(base_fn) dt_fn;
__typeof(base_fn) to_fn;

// A va_list handed on.
int vsum(int n, va_list ap);
inline int sum(int n, ...) {
  va_list ap;
  va_start(ap, n);
  int s = vsum(n, ap);
  va_end(ap);
  return s;
}

// A member of a base of an implicit instantiation, and its member class.
template <class T> struct counter {
  T n;
  T bump() { return ++n; }
};
template <int N> struct storage {
  struct type {
    alignas(N) unsigned char b[N];
  };
};
template <class T> struct holder : counter<T> {
  typename storage<sizeof(T)>::type s;
};
inline int use_holder() {
  holder<long> h{};
  h.bump();
  return int(h.bump()) + int(sizeof(h.s));
}
} // namespace tf
#endif
