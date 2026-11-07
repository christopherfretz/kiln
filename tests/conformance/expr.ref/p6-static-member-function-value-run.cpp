//@ expect: run
//@ stdout: 2 4 10 6 1 3 4
// [expr.ref]/6.3.1: `E1.f` / `E1->f` naming a static member function is an
// lvalue designating that function; E1 is still evaluated
// ([expr.ref]/2). Used as a value (not called) it decays to a function
// pointer: as a function-template argument, an initializer, a comparator,
// also for a static member function of a class template specialization.
#include <algorithm>
#include <cstdio>

template <class T> struct A {
  struct IV { T i; };
  static bool less(const IV& a, const IV& b);
  static int f(int);
};
template <class T> bool A<T>::less(const IV& a, const IV& b) { return a.i < b.i; }
template <class T> int A<T>::f(int x) { return x + 1; }

struct B { static int h(int x) { return x * 2; } };

int evaluated = 0;
B& next(B& b) { ++evaluated; return b; }

template <class F> int call(F f, int x) { return f(x); }

constexpr B cb{};
constexpr int (*cfp)(int) = cb.h;

int main() {
  A<int> a;
  B b;
  B* pb = &b;
  int (*p)(int) = b.h;
  auto q = pb->h;
  A<int>::IV v[3] = {{3}, {1}, {4}};
  std::sort(v, v + 3, a.less);
  int r = call(next(b).h, 3);
  std::printf("%d %d %d %d %d %d %d\n", call(a.f, 1), p(2), q(5), r, evaluated,
              v[1].i, cfp(2) == 4 ? v[2].i : -1);
  return 0;
}
