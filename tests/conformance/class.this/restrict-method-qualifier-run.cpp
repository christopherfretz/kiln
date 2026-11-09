// [expr.prim.this]/2 with GNU `__restrict` as a method qualifier: `this`
// is `T * __restrict` (`const T * __restrict` with `const`); the
// qualifier may be spelled `__restrict__`, combined with a
// ref-qualifier, repeated on an out-of-line definition, and used in a
// pointer-to-member type. A nullability keyword after the parameter list
// of a non-static member function or in a function type is accepted and
// dropped (clang; WP-1091).
//@ expect: run
//@ stdout: 22
#include <cstdio>
template<class A, class B> constexpr bool same = false;
template<class A> constexpr bool same<A, A> = true;
struct T {
  int x = 1;
  int ok() __restrict { static_assert(same<decltype(this), T* __restrict>); return x; }
  int c() const __restrict {
    static_assert(same<decltype(this), const T* __restrict>);
    return x + 1;
  }
  int r() __restrict__ & { return x + 2; }
  void n() _Nonnull {}
  int* p() _Nullable { return &x; }
  int out() __restrict;
  constexpr int ce() const __restrict { return 7; }
};
int T::out() __restrict { return this->x + 3; }
using F = void() _Nonnull;
using G = void() __restrict;
void (*fp)() _Nonnull = nullptr;
int pm(int (T::*m)() __restrict, T& t) { return (t.*m)(); }
int pm2(int (T::*m)() const __restrict, const T& t) { return (t.*m)(); }
static_assert(T{}.ce() == 7);
int main() {
  T t;
  const T ct;
  t.n();
  std::printf("%d\n", t.ok() + ct.c() + t.r() + t.out() + pm(&T::ok, t) + pm2(&T::c, ct) + *t.p()
                          + T{}.ce() + (fp == nullptr));
}
