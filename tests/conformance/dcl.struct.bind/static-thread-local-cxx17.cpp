//@ std: c++17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 16:1 warning "declared 'static' is a C++20 extension"
//@ diag: 17:1 warning "declared 'thread_local' is a C++20 extension"
//@ diag: 19:3 warning "declared 'static' is a C++20 extension"
//@ diag: 20:3 warning "declared 'thread_local' is a C++20 extension"
// C++20 [dcl.dcl]/8 (P1091R3): a structured binding declaration may be
// declared `static` or `thread_local` since C++20; in C++17 clang accepts
// it as an extension (`ext_decomp_decl_spec`, -Wc++20-extensions) at the
// specifier. Under -std=c++20 and later it is silent
// (static-thread-local-cxx20.cpp). clang 19 says "decomposition
// declaration", clang 22 "structured binding declaration".
struct S { int a, b; };
S s{1, 2};
static auto [a1, b1] = s;
thread_local auto [a2, b2] = s;
int f() {
  static auto [a3, b3] = s;
  thread_local auto [a4, b4] = s;
  return a1 + a2 + a3 + a4 + b1 + b2 + b3 + b4;
}
