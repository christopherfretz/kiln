// [basic.stc.dynamic.deallocation]/2: a deallocation function has at least
// one parameter, the first of type void* (or a destroying-delete form).
// Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 7:6 error "'operator delete' must have at least one parameter"
void operator delete();
