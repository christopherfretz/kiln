// Closures in a function template's signature (spec/abi.md §8.15,
// WP-1077; [expr.prim.lambda.closure]/2, [basic.link]/4.4). A lambda in
// the return type, a parameter type or an NTTP type is substituted with
// the signature and numbered in the template: a local entity of the
// pattern (`_ZZ3retT_ENKUlvE_clEv`, `ret<long>`'s closure `UlvE0_`). In a
// class template specialization's member it is numbered in that
// specialization (`nret`'s call operator is `_ZNK1SIiEUlvE1_clEv`, after
// the member template `mret`'s and the default argument of `mdta`'s). A
// default template argument's closure is numbered in the template's
// namespace or class. All are linkonce_odr, as are the specializations
// naming them; only the closures of the global namespace's default
// arguments (`dta`, `nval`) are internal, as in clang.
template <class T> auto ret(T) -> decltype([] { return 0; }) { return {}; }
template <class T> int par(T, decltype([] { return 1; }) = {}) { return 1; }
template <class T, class U = decltype([] { return 2; })> int dta(T) { return U{}(); }
template <class T, decltype([] { return 3; }) V = {}> int nttp(T) { return V(); }
template <class T, auto V = [] { return 4; }> int nval(T) { return V(); }
template <class T> struct S {
  template <class U> static auto mret(U) -> decltype([] { return 5; }) { return {}; }
  template <class U, class W = decltype([] { return 6; })> static int mdta(U) { return W{}(); }
  static auto nret(T) -> decltype([] { return 7; }) { return {}; }
};
namespace ns {
template <class T> auto ret(T) -> decltype([] { return 8; }) { return {}; }
template <class T, class U = decltype([] { return 9; })> int dta(T) { return U{}(); }
}
int use() {
  return ret(1)() + ret(1L)() + ret(2)() + par(1) + par(1L) + dta(1) + nttp(1) + nval(1)
       + S<int>::mdta(1) + S<int>::nret(1)() + ns::ret(1)() + ns::dta(1);
}
