// [class.temporary]/6-8, clang's [[clang::lifetimebound]] (WP-674): a
// reference or pointer obtained from a call through a lifetimebound
// parameter, implicit object parameter or constructor parameter refers
// to that argument, so binding a temporary argument dangles when the
// result outlives the full-expression: reference and pointer variables,
// returns (-Wreturn-stack-address), member initializers (-Wdangling-field,
// once per template) and assignments (-Wdangling-assignment). A local or
// parameter argument is fine where its lifetime suffices, and so is a
// static-storage reference bound at namespace scope to a lifetime-extended
// temporary. clang 19's analysis (clang 20+ diagnoses more: WP-674 follow-up).
//@ strict-diags
//@ std: c++23
//@ requires: clang<22
//@ diag: 51:22 warning "temporary bound to local reference 'r1' will be destroyed at the end of the full-expression"
//@ diag: 53:23 warning "temporary bound to local reference 'a' will be destroyed at the end of the full-expression"
//@ diag: 55:18 warning "temporary bound to local reference 'c' will be destroyed at the end of the full-expression"
//@ diag: 57:18 warning "temporary bound to local reference 'e' will be destroyed at the end of the full-expression"
//@ diag: 58:10 warning "temporary bound to local reference 's' will be destroyed at the end of the full-expression"
//@ diag: 59:24 warning "temporary whose address is used as value of local variable 'p' will be destroyed at the end of the full-expression"
//@ diag: 64:11 warning "temporary whose address is used as value of local variable 'v' will be destroyed at the end of the full-expression"
//@ diag: 66:13 warning "object backing the pointer q will be destroyed at the end of the full-expression"
//@ diag: 69:33 warning "returning reference to local temporary object"
//@ diag: 70:38 warning "reference to stack memory associated with parameter 'x' returned"
//@ diag: 71:44 warning "reference to stack memory associated with local variable 'y' returned"
//@ diag: 72:36 warning "reference to stack memory associated with local variable 's' returned"
//@ diag: 77:21 warning "binding reference member 'r' to stack allocated parameter 'x'"
//@ diag: 75:14 note "reference member declared here"
//@ diag: 77:37 warning "reference member 'p' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 76:14 note "reference member declared here"
//@ diag: 78:11 warning "reference member 'r' binds to a temporary object whose lifetime is shorter than the lifetime of the constructed object"
//@ diag: 75:14 note "reference member declared here"
//@ diag: 81:51 warning "temporary bound to local reference 't' will be destroyed at the end of the full-expression"
//@ diag: 82:15 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 83:24 warning "temporary bound to local reference 'ok3' will be destroyed at the end of the full-expression"
const int &pick(const int &a [[clang::lifetimebound]], const int &b);
struct S {
  int v;
  const int &get() const [[clang::lifetimebound]];
  const int &plain() const;
  S &self() [[clang::lifetimebound]];
};
struct W {
  W(const int &r [[clang::lifetimebound]]);
  const int *p;
};
struct V {
  V(const S &s [[clang::lifetimebound]]) : p(&s.v) {}
  const int *p;
};
S mk();
const int &r1 = pick(1, 2);
void f() {
  const int &a = pick(1, 2);
  const int &b = pick(a, 3);
  const int &c = S{1}.get();
  const int &d = S{1}.plain();
  const int &e = mk().get();
  S &s = mk().self();
  const int *p = &pick(4, 5);
  int x = 0;
  const int &ok = pick(x, 6);
  S local{1};
  const int &ok2 = local.get();
  V v = V(S{2});
  const int *q;
  q = &pick(7, 8);
  (void)a; (void)b; (void)c; (void)d; (void)e; (void)s; (void)p; (void)ok; (void)ok2; (void)v; (void)q;
}
const int &ret1() { return pick(1, 2); }
const int &ret2(int x) { return pick(x, 2); }
const int &ret3() { int y = 0; return pick(y, 2); }
const int &ret4() { S s{1}; return s.get(); }
const int &ret5(const int &z) { return pick(z, 2); }
struct M {
  const int &r;
  const int *p;
  M(int x) : r(pick(x, 1)), p(&pick(2, 3)) {}
  M() : r(S{1}.get()), p(nullptr) {}
};
template <class T> const T &tpick(const T &a [[clang::lifetimebound]]) { return a; }
template <class T> void tf() { const T &t = tpick(T(1)); (void)t; }
template void tf<int>();
const int &ok3 = tpick(5);
