// [expr.prim.id.general]/3, [class.mfct.non.static]/2: an overload set of
// non-static member functions (or member function templates) called
// without an object expression where no `this` may be used (a static
// member function) or where `*this` is of an unrelated class is "call to
// non-static member function without an object argument", diagnosed at
// the name before overload resolution (not as an ambiguous call); with
// template arguments and without. WP-543.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:27 error "call to non-static member function without an object argument"
//@ diag: 23:27 error "call to non-static member function without an object argument"
//@ diag: 26:23 error "call to non-static member function without an object argument"
//@ diag: 27:23 error "call to non-static member function without an object argument"
struct B {
  template <int I> int &get() { return v; }
  template <int I> const int &get() const { return v; }
  int &g() { return v; }
  const int &g() const { return v; }
  int v = 1;
};
struct D : B {
  static int s() { return get<0>(); }
  static int t() { return g(); }
};
struct E {
  int q() { return B::get<0>(); }
  int r() { return B::g(); }
};
