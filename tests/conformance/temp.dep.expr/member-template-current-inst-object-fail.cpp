// [temp.dep.expr]/5: a call of a member function template through an
// object of the current instantiation is resolved at instantiation, and
// a candidate that is not viable there (a const object, a non-const
// member) is diagnosed then, with the instantiation backtrace. Found
// running GCC's C++ testsuite (WP-1472).
//@ expect: compile-fail
//@ std: c++20
//@ diag: 11:47 error "no matching member function for call to 'f'"
template <class T> struct O {
  template <class U> int f() { return 0; }
  int a(O o) { return [o] { return o.template f<int>(); }(); }
};
int x = O<int>().a({});
