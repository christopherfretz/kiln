// [basic.start.main]/3: a `main` template that is already invalid (wrong
// return type) is not also reported as a template (clang's CheckMain;
// WP-1015).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:20 error "'main' must return 'int'"
template <class T> long main() { return 0; }
