// Mangling differential: closures in a function template's signature
// (spec/abi.md §8.15, WP-1077; [expr.prim.lambda.closure]/2). Each
// deduction that forms a new specialization substitutes the signature
// and numbers its lambda in the pattern: `ret<int>`'s closure is
// `_ZZ3retT_EUlvE_`, `ret<long>`'s `UlvE0_`, and the repeated `ret(2)`
// reuses `ret<int>`. A class template specialization's member numbers in
// the specialization (`nret`'s call operator is `_ZNK1SIiEUlvE1_clEv`),
// a default template argument in the template's namespace or class. The
// IR and linkage are tests/codegen/lambda_in_function_signature.cpp.
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
