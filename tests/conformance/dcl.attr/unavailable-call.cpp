// clang extension: a call to a function marked __attribute__((unavailable))
// is an error. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, the unavailable attribute is not enforced
//@ expect: compile-fail
//@ diag: 8:12 error "'f' is unavailable"
void f() __attribute__((unavailable));
void g() { f(); }
