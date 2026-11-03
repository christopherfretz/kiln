//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
//@ diag: 12:1 warning "declared 'static' is a C++20 extension"
//@ diag: 12:8 warning "declared 'thread_local' is a C++20 extension"
// C++20 [dcl.dcl]/8 (P1091R3), in C++17 mode: clang 22 reports `static`
// and `thread_local` on one structured binding declaration as two
// -Wc++20-extensions warnings, each at its specifier (clang 19's single
// combined warning: static-thread-local-both-cxx17-clang19.cpp).
struct S { int a, b; };
static thread_local auto [a, b] = S{1, 2};
int f() { return a + b; }
