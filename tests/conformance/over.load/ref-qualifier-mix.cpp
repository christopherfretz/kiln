// [over.load]/2.3: member function declarations with the same parameter
// types cannot be overloaded if some have a ref-qualifier and others do not.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 10:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
//@ diag: 9:8 note "previous declaration is here"
struct S {
  void f() &;
  void f();
};
