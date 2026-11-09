// [class.friend]/10 with [class.access]: a friend declaration naming a
// member function of another class must have access to it; a public member,
// or a private one of a class that befriends this one, is fine. A
// dependently-scoped one is checked per instantiation. Found by the clang
// negative-test run (WP-912), fixed by WP-1017.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:18 error "friend function 'f' is a private member of 'A'"
//@ diag: 15:8 note "implicitly declared private here"
//@ diag: 32:21 error "friend function 'h' is a private member of 'X<int>'"
//@ diag: 34:16 note "in instantiation of template class 'Y<int>' requested here"
//@ diag: 29:8 note "implicitly declared private here"
class A {
  void f();
  friend class C;

public:
  void g();
};
class B {
  friend void A::f();
  friend void A::g();
};
class C {
  friend void A::f();
};
template <class T> class X {
  void h();
};
template <class T> class Y {
  friend void X<T>::h();
};
template class Y<int>;
template <class T> class Z {
  friend void X<T>::h();
};
