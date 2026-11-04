// [over.match.funcs]/4-5, [over.ics.ref]/1: a member function template
// that only has a const overload, called by its template-id without an
// object expression from a non-const member function (`f<0>()`,
// `B::f<0>()`), binds the non-const `*this` to its `const B &` implicit
// object parameter: no diagnostic. WP-543.
//@ verify
// expected-no-diagnostics
struct B {
  template <int I> int f() const { return I; }
};
struct D : B {
  int g() { return f<0>() + B::f<1>(); }
  template <int I> int h() { return f<I>() + B::template f<I>(); }
};
int x = D().g() + D().h<2>();
