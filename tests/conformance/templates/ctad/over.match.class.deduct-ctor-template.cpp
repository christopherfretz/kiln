// [over.match.class.deduct]/1.2: each constructor template contributes a
// guide whose template parameters are the class template's followed by
// the constructor template's, and whose constraints are the conjunction
// of both.
//@ strict-diags
template<class T> struct X {
  template<class U> X(T, U);
};
X x(1, 2.0);
static_assert(__is_same(decltype(x), X<int>));
template<class T> struct Y {
  template<class It> Y(It, It);
};
Y<int> y0(1, 2);
template<class T, class U> struct P {
  template<class A, class B> P(A&&, B&&) requires (sizeof(A) > 1);
  P(T, U);
};
P p(1, 2.0);
static_assert(__is_same(decltype(p), P<int, double>));
template<class T> struct Q {
  template<class U> requires (sizeof(U) == 1) Q(T, U);
  template<class U> requires (sizeof(U) == 8) Q(T*, U);
};
Q q1(1, 'c');
static_assert(__is_same(decltype(q1), Q<int>));
int i;
Q q2(&i, 1.0);
static_assert(__is_same(decltype(q2), Q<int>));
template<class T> struct V { template<class... Us> V(T, Us...); };
V v(1, 2, 3.0);
static_assert(__is_same(decltype(v), V<int>));
template<class T> struct N { template<int I> N(T, int (&)[I]); };
int arr[3];
N n(1.0, arr);
static_assert(__is_same(decltype(n), N<double>));
template<class T> struct A {
  using size_type = unsigned long;
  template<class U> A(size_type, U, T);
};
A a(1ul, 2, 'c');
static_assert(__is_same(decltype(a), A<char>));
template<class... Ts> struct C {
  template<class U> C(U, Ts...);
};
C c(1.0, 2, 'x');
static_assert(__is_same(decltype(c), C<int, char>));
template<template<class> class TT> struct H {};
template<class T> struct W { template<template<class> class TT> W(H<TT>, T); };
template<class> struct Z {};
W w(H<Z>{}, 1);
static_assert(__is_same(decltype(w), W<int>));
template<class T> concept Small = sizeof(T) == 1;
template<class T> struct K {
  template<Small U> K(T, U);
};
K k(1, 'c');
static_assert(__is_same(decltype(k), K<int>));
template<class T> struct E { template<class U> explicit E(T, U); };
E e(1, 2);
static_assert(__is_same(decltype(e), E<int>));
template<class T> struct B { template<class U = int> B(T, U = U()); };
B b(1);
static_assert(__is_same(decltype(b), B<int>));
