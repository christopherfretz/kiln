// [temp.friend]/5: clang 19's -Wunsupported-friend spelling (the specifier
// with its trailing `::`); the clang 22 shape and the rule are
// dependent-member-friend-warning.cpp.
//@ requires: clang<22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 32:41 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 33:42 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 34:41 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 35:44 warning "dependent nested name specifier 'A<T>::X::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 36:43 warning "dependent nested name specifier 'A<T *>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 37:39 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 38:43 warning "dependent nested name specifier 'A<T>::D::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 39:58 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 40:37 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 46:41 warning "dependent nested name specifier 'A<T>::' for friend class declaration is not supported; turning off access control for 'Outer'"
template <class T> struct A {
  struct Member { static int get(); };
  struct X { struct Y {}; };
  struct D { void g(); };
  template <class V> static int h();
  T i();
  static int f();
};
template <class T> struct A<T *> {
  struct Member {};
};
template <class T> struct C {
  struct Member {};
};
class B {
  template <class T> friend class A<T>::Member;
  template <class T> friend struct A<T>::Member;
  template <class T> friend union A<T>::Member;
  template <class T> friend class A<T>::X::Y;
  template <class T> friend class A<T *>::Member;
  template <class T> friend int A<T>::f();
  template <class T> friend void A<T>::D::g();
  template <class T> template <class V> friend int A<T>::h();
  template <class T> friend T A<T>::i();
  friend class A<int>::Member;
  template <class T> friend class C;
  static const int n = 7;
};
template <class U> struct Outer {
  template <class T> friend class A<T>::Member;
  friend class C<U>::Member;
  static const int n = 7;
};
Outer<int> o;
