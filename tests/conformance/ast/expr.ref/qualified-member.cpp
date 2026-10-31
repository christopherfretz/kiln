// [expr.ref]/4: a qualified member access records its
// nested-name-specifier; on a dependent object or qualifier it is a
// CXXDependentScopeMemberExpr.
//@ expect: ast
struct B {
  int m;
  virtual int f() { return 1; }
};
struct D : B {
  int m;
  int f() override { return 2; }
  int g() { return this->B::f() + this->D::m + B::m; }
};
template <class T> int dep(T t) { return t.Base::m + t.T::f() + t.B::m; }
template <class T> struct TD : T {
  int h() { return this->T::m + this->TD::h(); }
};
template <class T> struct TB {
  T t;
  int k() { return t.B::f() + t.::B::m; }
};
int use(D d, D* p) { return d.B::m + p->B::f() + d.::B::m + p->D::m; }
