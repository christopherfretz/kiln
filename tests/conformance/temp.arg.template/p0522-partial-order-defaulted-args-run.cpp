// [temp.func.order]/3, [temp.deduct.type]/8 with CWG2398 as clang 19 and
// clang 22 deduce it: in the partial ordering of function templates,
// `U<T>` (`template<class> class U`) is deduced from the transformed
// `num<B', E'>` with `E'` taken as `U`'s default, while `num<B, E>` is not
// deduced from `U'<T'>`, so `f(const num<B, E>&)` is more specialized
// than `f(const U<T>)`. Boost.Math's `sinhc_pi(const U<T>)` against
// Boost.Multiprecision's `sinhc_pi(const number<B, ET>&)`
// (test_reverse_mode_autodiff_sinhc_pi_compile_test) was ambiguous
// (WP-812).
//@ expect: run
//@ stdout: 1 1 1 1 1
extern "C" int printf(const char*, ...);
template<class B> struct dflt { static constexpr int value = 0; };
template<class B, int E = dflt<B>::value> struct num { B v; };
template<class B, class C = int> struct two { B v; };
template<class X> struct box {};
template<class B, int E> int f(const num<B, E>&) { return 1; }
template<class T, template<class> class U> int f(const U<T>) { return 2; }
template<class B, class C> int g(two<B, C>) { return 1; }
template<class T, template<class> class U> int g(U<T>) { return 2; }
template<class B, int E> int h(box<num<B, E>>) { return 1; }
template<class T, template<class> class U> int h(box<U<T>>) { return 2; }

int main() {
  num<double> a{1.0};
  num<double, 1> b{1.0};
  two<int> c{2};
  printf("%d %d %d %d %d\n", f(a), f(b), g(c), h(box<num<int>>()), h(box<num<int, 3>>()));
}
