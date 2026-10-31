//@ expect: ast
// [temp.friend]/1: class templates that befriend themselves and each
// other. Each befriending is a redeclaration of the class template inside
// the befriending class. Regression test (WP-121): the dump once followed
// the friend back to the enclosing template and grew without bound.
template <class T> class A;
template <class T> class B {
  template <class U> friend class A;
  template <class U> friend class B;
  T b;
};
template <class T> class A {
  template <class U> friend class B;
  template <class U> friend class A;
  T a;
};
template <class T> class C {
  template <class U> friend class C;
  template <class V> friend class C;
};
