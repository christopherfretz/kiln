//@ expect: ast
//@ skip(clang): clang 22 treats a call through `this` of the current instantiation as type-dependent
//@ oracle: none
// [temp.dep.expr]/3, /5, [temp.dep.type]/7: which calls naming members of
// the current instantiation get their type at definition (WP-1609). `g`:
// an overload set, a member template whose deduced specialization is not
// dependent, a static set named `A::`, explicit template arguments: the
// return type. `h`: everything that stays `<dependent type>`: a member
// template whose result (`dep`), default template argument (`dflt`) or
// parameter (`ptp`) names `T`, a constrained one (`con`, `rq`), a deduced
// return type (`ded`), an explicit object parameter (`ex`), a template
// template parameter (`tt`), a candidate whose type names `T` (`ov`), a
// type-dependent argument, an ambiguous or non-viable call (diagnosed at
// instantiation, as clang does), a conversion function name, and every
// call in a class with a dependent base (`D`). Not yet resolved at
// definition (WP-1628): an unqualified static set `so(1)` and an
// unqualified template-id `tm<long>(1)`.
template <class> concept C = true;
template <class> struct B { int fb(int); };
template <class T> struct A {
  int st(int); int st(long);
  static int so(int); static int so(long);
  template <class U> int tm(U);
  template <class U> T dep(U);
  template <class U, class V = T> int dflt(U);
  template <class U> int ptp(U, T);
  template <C U> int con(U);
  template <class U> int rq(U) requires true;
  template <class U> auto ded(U) { return 1; }
  template <class U> int ex(this U, int);
  template <template <class> class X> int tt(X<int>);
  int ov(int); int ov(T);
  operator int();
  void g() {
    st(1);
    tm(1);
    A::so(1);
    this->tm<long>(1);
  }
  void h(T t) {
    dep(1);
    dflt(1);
    ptp(1, 2);
    con(1);
    rq(1);
    ded(1);
    ex(1);
    tt(B<int>());
    ov(1);
    st(t);
    st(1.0);
    st("");
    this->operator int();
    so(1);
    tm<long>(1);
  }
};
template <class T> struct D : B<T> {
  int g(int); int g(long);
  void f() { g(1); this->g(1); }
};
