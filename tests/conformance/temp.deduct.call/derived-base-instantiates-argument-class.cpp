// [temp.deduct.call]/4.3, [temp.inst]/2 (WP-682): when P is a
// simple-template-id and A a class template specialization that nothing
// has instantiated yet, deduction instantiates A before it tries A's base
// classes. The calls are qualified, so no argument-dependent lookup
// completes the class first. libstdc++ 14 `std::get<0>(t)` on a
// `std::tuple<int, int>&` (tuple:2446, `std::__get_helper<__i>(__t)`) is
// the second shape (`auto` makes the body instantiate at the call).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 3 4 9 5
#include <stdio.h>

template<class T> struct B { T v; };
template<class T> struct D : B<T> {};
template<class T> T &f(B<T> &b) { return b.v; }
template<class T> T *fp(B<T> *b) { return &b->v; }
int by_ref(D<int> &d) { return ::f(d); }
long *by_ptr(D<long> *p) { return ::fp(p); }

namespace n {
template<unsigned long I, class... T> struct Impl;
template<unsigned long I, class H, class... T>
struct Impl<I, H, T...> : Impl<I + 1, T...> { H h; };
template<unsigned long I, class H> struct Impl<I, H> { H h; };
template<class... T> struct Tup : Impl<0, T...> {};

template<unsigned long I, class H, class... T>
H &helper(Impl<I, H, T...> &t) { return t.h; }
template<unsigned long I, class H, class... T>
auto &get(Tup<H, T...> &t) { return n::helper<I>(t); }
} // namespace n
int first(n::Tup<int, long> &t) { return n::get<0>(t); }

int main() {
  D<int> d;
  d.v = 3;
  D<long> e;
  e.v = 4;
  n::Tup<int, long> t;
  n::get<0>(t) = 9;
  t.n::Impl<1, long>::h = 5;
  printf("%d %ld %d %ld\n", by_ref(d), *by_ptr(&e), first(t), t.n::Impl<1, long>::h);
}
