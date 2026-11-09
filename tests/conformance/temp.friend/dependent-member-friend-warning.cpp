// [temp.friend]/5: clang 22's -Wunsupported-friend, emitted once per
// friend declaration with its own template head whose nested-name-
// specifier is dependent, at the befriended name, never per instantiation
// and not for `friend class A<int>::Member;` or `friend class C<U>::Member;`
// (no own head). kiln models the friend instead of turning off access
// control, but keeps clang's text. The clang 19 spelling (`'A<T>::'`) is
// dependent-member-friend-warning-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ diag: 36:41 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 37:42 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 38:41 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 39:44 warning "dependent nested name specifier 'A<T>::X' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 40:43 warning "dependent nested name specifier 'A<T *>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 41:39 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 42:43 warning "dependent nested name specifier 'A<T>::D' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 43:58 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 44:37 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'B'"
//@ diag: 50:41 warning "dependent nested name specifier 'A<T>' for friend class declaration is not supported; turning off access control for 'Outer'"
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
