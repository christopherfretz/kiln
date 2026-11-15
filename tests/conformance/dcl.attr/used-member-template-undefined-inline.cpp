// GNU used on a member function or member function template of a class
// template: instantiating the class (explicitly or implicitly) uses the
// member, so an inline one never defined is -Wundefined-inline at the
// class's point of instantiation (clang). A member with a body (h) and a
// non-template class (B) are not diagnosed.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 17:56 warning "inline function 'A<int>::f' is not defined"
//@ diag: 21:17 note "used here"
//@ diag: 18:37 warning "inline function 'A<int>::g' is not defined"
//@ diag: 21:17 note "used here"
//@ diag: 17:56 warning "inline function 'A<char>::f' is not defined"
//@ diag: 22:9 note "used here"
//@ diag: 18:37 warning "inline function 'A<char>::g' is not defined"
//@ diag: 22:9 note "used here"
template <class T> struct A {
  template <class U> __attribute__((used)) inline void f();
  __attribute__((used)) inline void g();
  __attribute__((used)) void h() {}
};
template struct A<int>;
A<char> a;
struct B {
  template <class U> __attribute__((used)) inline void f();
  __attribute__((used)) inline void g();
};
