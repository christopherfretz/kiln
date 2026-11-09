// [basic.start.main]/2: clang accepts `const` at any level of `main`'s
// `char**` parameters and a function-try-block body (WP-1015).
//@ std: c++26
//@ expect: compile-ok
int main(const int, const char *const *const) try { return 0; } catch (...) { return 1; }
