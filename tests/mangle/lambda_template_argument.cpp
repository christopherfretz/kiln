// Mangling differential: lambdas in template parameters and template
// arguments (spec/abi.md §8.15, WP-1062; [temp.param]/10,
// [expr.prim.lambda.closure]/2). A lambda-expression in a dependent
// signature is `L <closure type> E`, the closure unnumbered (`$_N`,
// numbered in order of mangling). A default template argument's lambda is
// a new closure at each use, numbered then per <lambda-sig> in the
// template's namespace or class: two uses of `f` call `f<UlvE_>` and
// `f<UlvE0_>`, `N::a` and `S<int>::m` take `N`'s and `S<int>`'s numbers
// (`_ZN1N1aITnDaXtlNS_UlvE_EEEEEiv`, `_ZN1SIiE1mINS0_UlvE_EEEiv`).

template <decltype([] { return 12; }()) N> int nttp() { return N; }
template <class T = decltype([] {})> int f() { return 1; }
template <class T = decltype([](int) {})> int f2() { return 2; }
namespace N { template <auto F = [] { return 7; }> int a() { return F(); } }
template <class T> struct S { template <class U = decltype([] {})> static int m() { return 1; } };
struct R { template <int I = [](int x) { return x; }(4)> static int d() { return I; } };
template <class T, class U = decltype([](T) {})> int b(T) { return 2; }
template <class T = decltype([] {})> struct C { static int v() { return 3; } };

int use() {
  return nttp<12>() + f() + f() + f2() + N::a() + N::a() + S<int>::m() + S<int>::m()
       + S<long>::m() + R::d() + b(1) + b(1L) + C<>::v();
}
