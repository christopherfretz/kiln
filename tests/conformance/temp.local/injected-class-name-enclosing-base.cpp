// [temp.local]/1: in a member class (or member class template) of a class
// template defined outside it, the unqualified injected-class-name of the
// enclosing template names its current specialization, so it is a valid
// base-specifier. Found running GCC's C++ testsuite (WP-1467).
template <class T> struct A {
  int x = 1;
  struct C;
  template <class U> struct M;
};
// Out of the template's definition, the injected-class-name `A` still
// names the enclosing class template specialization.
template <class T> struct A<T>::C : A { int f() { return x; } };
template <class T> template <class U> struct A<T>::M : A { int g() { return A::x; } };
A<int>::C c;
A<int>::M<char> m;
int r = c.f() + m.g();
