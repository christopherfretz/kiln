// [temp.param]/12: a default template argument shall not be specified in a
// friend template declaration that is not a definition. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, default template arguments on friend templates are accepted
//@ expect: compile-fail
//@ diag: 9:19 error "default template argument not permitted on a friend template"
struct S {
  template <class T = int> friend void f();
};
