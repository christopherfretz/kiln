// [expr.prim.id.qual]/1: a `decltype(e)` qualifier is kept as written in
// the type and expression sugar (`decltype(s)::type`, `typename
// decltype(s)::type`, `int decltype(s)::*`); in a template it is a
// dependent nested-name-specifier (a `DependentScopeDeclRefExpr`, a
// `typename decltype(T())::type`), rebuilt when instantiated.
//@ expect: ast
struct S {
  using type = int;
  static constexpr int v = 3;
  template <class T> struct M { using type = T; };
  int m;
};
S s;
decltype(s)::type y = 2;
typename decltype(s)::type x = 1;
decltype(s)::M<int>::type z = 3;
int decltype(s)::*pm = &S::m;
int k = decltype(s)::v;
template <class T> struct X {
  static const int value = decltype(T())::v;
  using type = typename decltype(T())::type;
};
int q = X<S>::value;
X<S>::type r = 4;
