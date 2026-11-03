//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang<22
//@ diag: 11:1 warning "declaration declared with 'static thread_local' specifiers is a C++20 extension"
// C++20 [dcl.dcl]/8 (P1091R3), in C++17 mode: clang 19 reports `static`
// and `thread_local` on one structured binding declaration as a single
// -Wc++20-extensions warning at the first specifier (clang 22: one per
// specifier, static-thread-local-both-cxx17.cpp).
struct S { int a, b; };
static thread_local auto [a, b] = S{1, 2};
int f() { return a + b; }
