// [class.dtor]/1: a destructor declarator-id written as a template-id must
// name the class itself: `~Y<int>()` in `Y<T>` is an error (GCC 16.1 and
// clang 22). Found running GCC's C++ testsuite
// (WP-1466).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 10:30 error "expected the class name after '~' to name the enclosing class"
template<class T> struct X {};
template<class T> struct Y { ~Y<int>(); };
