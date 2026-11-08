// [temp.arg.template]/3 (P0522), [temp.deduct.type]/8: `U<T>` with
// `template<class> class U` is deduced from `rv<double, 1>` of
// `template<class, size_t = 1> struct rv`, since the trailing argument is
// its default; `rv<double, 2>` does not match and the generic overload is
// chosen. Boost.Math's `sinhc_pi(const U<T>)` taken for its reverse-mode
// `rvar<T, 1>` (test_reverse_mode_autodiff_sinhc_pi_compile_test) relies on
// it; kiln picked `sinhc_pi(T)` instead (WP-812).
//@ expect: run
//@ stdout: 2 1 2 1 2 3 4
extern "C" int printf(const char*, ...);
using size_t = decltype(sizeof 0);
template<class T, size_t N = 1> struct rv { T v; };
template<class T, class U = int> struct pr { T v; };
template<class T> int f(T) { return 1; }
template<class T, template<class> class U> int f(const U<T>) { return 2; }
template<template<class> class U> int g(U<int>) { return 3; }
template<class T, template<class> class U> U<T> h(U<T> x) { return x; }

int main() {
  rv<double, 1> a{1.0};
  rv<double, 2> a2{1.0};
  rv<int> b{2};
  pr<int, long> c{3};
  pr<int> e{4};
  printf("%d %d %d %d %d %d %d\n", f(a), f(a2), f(b), f(c), f(e), g(b), int(sizeof(h(e))));
}
