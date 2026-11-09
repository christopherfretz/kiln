// [basic.start.main]/3: main shall not be a function template. Found by the
// clang negative-test run (WP-912).
//@ std: c++20
//@ xfail(kiln): WP-912 follow-up, main restrictions are not checked
//@ expect: compile-fail
//@ diag: 7:24 error "'main' cannot be a template"
template <class T> int main() { return 0; }
