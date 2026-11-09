// [namespace.udecl]/10: dependent using-declarators naming the same member
// name through different qualifiers, or different names, are not
// redeclarations.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template<class T> struct B { int i = 1; void f() {} };
template<class T> struct C { int j = 2; void f(int) {} };
template<class T> struct E : B<T>, C<T> {
  using B<T>::i;
  using C<T>::j;
  using B<T>::f;
  using C<T>::f;
};
template<class T, class U> struct H : T, U {
  using T::f;
  using U::f;
};
int use() {
  E<int> e;
  e.f();
  e.f(0);
  H<B<int>, C<int>> h;
  h.f();
  h.f(0);
  return e.i + e.j;
}
