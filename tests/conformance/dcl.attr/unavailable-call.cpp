// clang extension: a call to a function marked __attribute__((unavailable))
// is an error. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:12 error "'f' is unavailable"
//@ diag: 8:6 note "'f' has been explicitly marked unavailable here"
void f() __attribute__((unavailable));
void g() { f(); }
