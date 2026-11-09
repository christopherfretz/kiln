// [basic.start.main]/3: a program that declares a variable main at global
// scope is ill-formed. Found by the clang negative-test run (WP-912).
//@ std: c++20
//@ expect: compile-fail
//@ diag: 6:1 error "main cannot be declared as "
int main;
