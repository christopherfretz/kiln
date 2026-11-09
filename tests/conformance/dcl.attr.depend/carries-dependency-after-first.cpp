// [dcl.attr.depend]/2 (C++20): carries_dependency shall appear on the first
// declaration of a function if it appears on any. Found by the clang
// negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, carries_dependency placement is not checked
//@ expect: compile-fail
//@ diag: 9:3 error "function declared '[[carries_dependency]]' after its first declaration"
void f(int);
[[carries_dependency]] void f(int);
