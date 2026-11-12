// Mangling differential: a pointer or reference to a variable-length
// array parameter type (a Clang extension in C++). clang's canonical
// parameter type replaces every bound by `*` (`A_ <type>`), and no two
// `VariableArrayType`s are the same type, so a repeated one is spelled
// again rather than substituted (`PA_iPA_i`). It used to mangle as
// `u7unknown` (WP-1355 audit).
#pragma clang diagnostic ignored "-Wvla-cxx-extension"
void f(int n, int (*p)[n]) {}
void g(int n, const int (*p)[n][4], int (*q)[n][n], int (*r)[n]) {}
void k(int n, double (&r)[n], double (&s)[n]) {}
void m(int n, int m, int (*q)[n], int (*r)[m], int *s, int *t) {}
