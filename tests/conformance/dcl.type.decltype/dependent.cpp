// [temp.dep.type]/10: decltype of a type-dependent expression is a
// dependent type, resolved at instantiation.
//@ strict-diags
template <class T> struct W {
  static T v;
  static T &lv();
  using A = decltype(v);
  using B = decltype((v));
  using C = decltype(lv());
  using D = decltype(v + 1);
};
static_assert(__is_same(W<int>::A, int) && __is_same(W<int>::B, int &));
static_assert(__is_same(W<char>::C, char &) && __is_same(W<char>::D, int));
static_assert(__is_same(W<const long>::B, const long &));
