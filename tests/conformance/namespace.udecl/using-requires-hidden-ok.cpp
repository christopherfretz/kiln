// [namespace.udecl]/14: a member hides the base member a
// using-declarator names when their trailing requires-clauses
// correspond: the same class arguments (B<T> : A<T>), or a constraint
// on the member template's own parameter only (C<T> : A<T *>).
//@ std: c++20
//@ expect: run
template <class T> concept Any = true;
template <class T> struct A {
  template <class U> int f(U) requires Any<T> { return 1; }
  int g() requires Any<T> { return 1; }
  template <class U> int h(U) requires Any<U> { return 1; }
};
template <class T> struct B : A<T> {
  using A<T>::f;
  using A<T>::g;
  using A<T>::h;
  template <class U> int f(U) requires Any<T> { return 2; }
  int g() requires Any<T> { return 2; }
  template <class U> int h(U) requires Any<U> { return 2; }
};
template <class T> struct C : A<T *> {
  using A<T *>::h;
  template <class U> int h(U) requires Any<U> { return 3; }
};
int main() {
  B<int> b;
  C<int> c;
  return b.f(1) + b.g() + b.h(1) + c.h(1) == 9 ? 0 : 1;
}
