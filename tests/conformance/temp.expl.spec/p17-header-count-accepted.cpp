// The accepted side of p17-header-count-member-of-implicit-specialization.cpp:
// one `template<>` for a non-template member of an implicit
// specialization, two for a member template, one for a member template of
// an explicitly specialized class, and a non-template overload chosen
// over a member template of the same name.
//@ expect: compile-ok
//@ args: -std=c++20
template <class T> struct A {
  template <class U> void f(U);
  void f2(int);
  template <class U> void h(U);
  void h(long);
};
template <> struct A<float> { template <class U> void g(U); void g2(float); };
template <> void A<int>::f2(int) {}
template <> template <> void A<int>::f(int) {}
template <> template <> void A<int>::f<char>(char) {}
template <> template <class U> void A<char>::f(U) {}
template <> void A<float>::g(float) {}
template <> void A<float>::g<int>(int) {}
void A<float>::g2(float) {}
template <> void A<long>::h(long) {}
template <> template <> void A<long>::h(long) {}
template <class T> struct O { template <class U> struct I { void g(); }; };
template <> template <> void O<int>::I<int>::g() {}
