// [dcl.attr.nodiscard], [dcl.attr.deprecated], [temp.inst]/3: attributes
// written only on an out-of-line definition of a member template of a
// class template (WP-837). The class's instantiation (`A<int> a;`)
// instantiates the member's declaration; a definition after it adds
// nothing to the specialization, so its `nodiscard` and `deprecated` are
// not diagnosed at a use, as in clang. A definition before the class is
// instantiated (`B`) is merged first and applies; an attribute on the
// in-class declaration always applies.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 32:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 33:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
template<class T> struct A {
  template<class U> int f(U);
  template<class U> int g(U);
  template<class U> [[nodiscard]] int h(U);
};
A<int> a;
template<class T> template<class U> [[nodiscard]] int A<T>::f(U) { return 1; }
template<class T> template<class U> [[deprecated]] int A<T>::g(U) { return 2; }
template<class T> template<class U> int A<T>::h(U) { return 3; }

template<class T> struct B {
  template<class U> int f(U);
};
template<class T> template<class U> [[nodiscard]] int B<T>::f(U) { return 4; }
B<int> b;

void use() {
  a.f(1);
  a.g(1);
  a.h(1);
  b.f(1);
}
