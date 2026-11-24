// Mangling differential: lambdas in a lambda's parameter-declaration-clause
// (spec/abi.md §8.9, WP-2627; [expr.prim.lambda.closure]/2,
// [dcl.fct.default]/3). A lambda-expression in another lambda's parameter
// type or default argument, as written, is numbered with no context
// declaration: in an inline or templated enclosing function only, never
// in an enclosing variable's, data member's or member function default
// argument's context (clang 19 and 22). So `f1`'s, `C::sm`'s and
// `C::fm`'s inner closures are `$_N` (internal), `fn`'s are numbered in
// `fn` (`_ZZ2fnvENKUlvE_clEv`). Where the enclosing lambda is substituted
// (a variable template, a function template, a generic lambda's
// specialization) its default argument is instantiated for the new call
// operator's parameter and the closure is numbered per that parameter
// (`_ZZNK2vtIiEMUliE_clEiEd_NKUlvE_clEv`,
// `_ZZZ2fnvENKUlT_iE_clIiEEDaS_iEd_NKUlvE_clEv`).

inline auto f1 = [](int x = [] { return 1; }()) { return x; };
inline auto f2 = [](decltype([] { return 2; }) p = {}) { return p(); };
template <class T> auto vt = [](int x = [] { return 3; }()) { return x + T(); };
struct C {
  static inline auto sm = [](int x = [] { return 4; }()) { return x; };
  int fm = [](int x = [] { return 5; }()) { return x; }();
  void mf(int a = [](int x = [] { return 6; }()) { return x; }()) {}
};
inline int fn() {
  auto l = [](int y = [] { return 7; }()) { return y; };
  auto gl = [](auto a, int z = [] { return 8; }()) { return a + z; };
  auto nn = [](int p = [](int q = [] { return 9; }()) { return q; }()) { return p; };
  auto d = [](decltype([] { return 10; }) p = {}) { return p(); };
  auto w = [] { return 11; };
  struct L { int m(int r = [] { return 12; }()) { return r; } };
  return l() + gl(1) + gl(2.0) + gl(3) + nn() + d() + w() + L().m();
}
template <class T> int tf() {
  auto t = [](T u = [] { return T(13); }()) { return u; };
  auto o = [](int x = [](int y = [] { return 14; }()) { return y; }()) { return x; };
  auto gl = [](auto a, int z = [] { return T(15); }()) { return a + z; };
  auto w = [] { return 16; };
  struct L { int m(int r = [] { return 17; }()) { return r; } };
  return t() + o() + gl(1) + w() + L().m();
}

int use() {
  C c;
  c.mf();
  return f1() + f2() + vt<int>() + C::sm() + c.fm + fn() + tf<int>();
}
