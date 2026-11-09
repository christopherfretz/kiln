// [over.load]/2.3: member function declarations with the same parameter
// types cannot be overloaded if some have a ref-qualifier and others do not.
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, ref-qualifier overload consistency is not checked
//@ expect: compile-fail
//@ diag: 10:8 error "cannot overload a member function without a ref-qualifier with a member function with ref-qualifier '&'"
struct S {
  void f() &;
  void f();
};
