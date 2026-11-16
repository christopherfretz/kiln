// [temp.names]/1,5, [temp.explicit]: a dependent qualified operator function
// template-id (`&T::template operator-<double>`) names the member operator
// template after substitution, so the explicit instantiation that deduces
// through it matches. Found running GCC's C++ testsuite (WP-1467).
struct A {
  template <class T> int f();
  int operator+();
  operator int();
  template <class T> int operator-();
};
typedef int (A::*P)();
template <P> struct S {};
template <class T> void g(S<&T::template f<int> >) {}
template <class T> void g(S<&T::operator+>) {}
template <class T> void g(S<&T::operator int>) {}
template <class T> void g(S<&T::template operator-<double> >) {}
template void g<A>(S<&A::f<int> >);
template void g<A>(S<&A::operator+>);
template void g<A>(S<&A::operator int>);
template void g<A>(S<&A::operator-<double> >);
