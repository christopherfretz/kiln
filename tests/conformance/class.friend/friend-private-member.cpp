// [class.friend]/10 with [class.access]: a friend declaration naming a
// member function of another class must have access to it. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, access to a befriended member function is not checked
//@ expect: compile-fail
//@ diag: 12:18 error "friend function 'f' is a private member of 'A'"
class A {
  void f();
};
class B {
  friend void A::f();
};
