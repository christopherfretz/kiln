// _Generic (C11 6.5.1.1, a clang extension in C++) in a template: a
// type-dependent controlling operand or association type makes the
// selection `result_dependent` (no association is `selected`, the type is
// `<dependent type>`); each instantiation selects (WP-1131,
// [temp.dep.expr]). `_Generic(T(), ...)` would be the type-operand form
// with the function type `T ()` ([dcl.ambig.res]), hence `T{}`.
//@ expect: ast
//@ oracle: clang
template <class T> struct S {
  static constexpr int v = _Generic(T{}, int: 1, long: 2, default: 3);
  static constexpr int w = _Generic(T, int: 10, default: 30);
  static constexpr int d = _Generic(0, T: 100, default: 300);
  static constexpr int n = _Generic(T{}, default: 7);
  static constexpr int k = _Generic(0, int: 5, default: 6);
};
int s = S<int>::v + S<int>::w + S<int>::d + S<int>::n + S<int>::k;
int l = S<long>::v + S<long>::w + S<long>::d;
