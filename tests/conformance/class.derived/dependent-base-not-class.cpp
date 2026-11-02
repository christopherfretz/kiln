// [class.derived.general]/2: a base-specifier that does not name a class
// after substitution is dropped and the instantiated class is invalid, so
// later uses of it are not diagnosed again.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:31 error "base specifier must name a class"
//@ diag: 12:8 note "in instantiation of template class 'D<int>' requested here"
template <class T> struct D : T {
  int m;
  void g() { m = 1; }
};
D<int> d;
int x = d.m;
int y = sizeof(D<int>);
