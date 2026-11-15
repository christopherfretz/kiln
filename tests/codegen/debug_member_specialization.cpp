// Debug info (WP-1445): the declaration DISubprogram of an explicitly
// specialized member of a class template specialization, and of an
// explicit specialization of a member function template, is at the
// specialization, not at the template's member (clang
// `CheckMemberSpecialization` moves the instantiated member there).
//@ args: -g
template <class T> struct A {
  void f();
  static int g(int);
  template <class U> void h(U);
  void k();
};
template <class T> void A<T>::f() {}
template <class T> void A<T>::k() {}

template <> void A<int>::f() {}
template <>
int A<int>::g(int x) { return x; }
template <> template <> void A<int>::h(long) {}
void use() {
  A<int> a;
  a.f();
  A<int>::g(1);
  a.h(2L);
  a.k();
  A<char>().f();
}
