// Attributes written only on an out-of-line definition of a member
// template of a class template (WP-837, [temp.inst]/3): clang instantiates
// the member's declaration with its class, so a definition that follows
// that instantiation (`A<int> a;` before it) adds nothing to the
// specialization: `pure`, `const` and `cold` are dropped there and the
// calls carry no `memory(...)`. A definition that precedes the class's
// instantiation (`B<int>` below) is merged into the declaration first, so
// its attributes apply. An attribute on the in-class declaration always
// applies.
template<class T> struct A {
  template<class U> int f(U);
  template<class U> int g(U);
  template<class U> int k(U);
  template<class U> [[gnu::cold]] int m(U);
};
A<int> a;
template<class T> template<class U> [[gnu::pure]] int A<T>::f(U) { return 1; }
template<class T> template<class U> [[gnu::const]] int A<T>::g(U) { return 2; }
template<class T> template<class U> [[gnu::cold]] int A<T>::k(U) { return 3; }
template<class T> template<class U> int A<T>::m(U) { return 4; }
template<class T> struct B { template<class U> int f(U); template<class U> int g(U); };
template<class T> template<class U> [[gnu::pure]] int B<T>::f(U) { return 5; }
template<class T> template<class U> [[gnu::const]] int B<T>::g(U) { return 6; }
B<int> b;
int h() {
  return a.f(1) + a.g(2) + a.k(3) + a.m(4) + b.f(5) + b.g(6);
}
