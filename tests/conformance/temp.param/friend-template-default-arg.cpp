// [temp.param]/12: a default template argument shall not be specified in a
// friend template declaration that is not a definition. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 8:19 error "default template argument not permitted on a friend template"
struct S {
  template <class T = int> friend void f();
};
