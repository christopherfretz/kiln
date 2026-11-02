// [temp.dep.type]/1, [temp.res.general]/4-5: a class declared in a
// dependent context (a local class of a function template, a member class
// of a class template, a local class of such a member) is the current
// instantiation only inside its own definition. Elsewhere `L::U` is a
// dependent qualified name, so a block-scope declaration, condition,
// for-init-statement or block-scope typedef needs `typename` before it
// (clang "missing 'typename'", parsed as if written). Member declarations
// and parameters of member functions imply `typename`; a class in a
// non-dependent context is never dependent. WP-328: kiln accepted these.
// Not covered: clang parses `L::U *p;` and `sizeof(L::U)` as expressions
// (kiln: types; see spec/sema-templates.md §5.9).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:3 error "missing 'typename' prior to dependent type name 'L::U'"
//@ diag: 30:7 error "missing 'typename' prior to dependent type name 'L::U'"
//@ diag: 31:8 error "missing 'typename' prior to dependent type name 'L::U'"
//@ diag: 32:11 error "missing 'typename' prior to dependent type name 'L::U'"
//@ diag: 39:14 error "missing 'typename' prior to dependent type name 'N::U'"
//@ diag: 42:37 error "missing 'typename' prior to dependent type name 'N::U'"
//@ diag: 51:5 error "missing 'typename' prior to dependent type name 'L::U'"
template <class T> int f() {
  struct L {
    using U = T;
    struct M { L::U x; };
    L::U y;
    void g(L::U) {}
    int m() { L::U w = 0; return w; }
  };
  L::U z = 0;
  if (L::U c = 1) {}
  for (L::U i = 0; i < 1; ++i) {}
  typedef L::U V;
  return z + V() + sizeof(typename L::U) + L().m();
}
int x = f<int>();
template <class T> struct A {
  struct N { using U = T; };
  N::U y;
  void h() { N::U w; (void)w; }
  void k();
};
template <class T> void A<T>::k() { N::U v; (void)v; }
template struct A<int>;
void nt() {
  struct L { using U = int; };
  L::U z = 0; (void)z;
}
template <class T> struct B {
  void h() {
    struct L { using U = int; };
    L::U z = 0; (void)z;
  }
};
