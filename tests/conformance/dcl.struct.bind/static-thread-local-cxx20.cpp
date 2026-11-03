//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
// C++20 [dcl.dcl]/8 (P1091R3): `static` and `thread_local` structured
// binding declarations are standard C++20; no -Wc++20-extensions warning
// (static-thread-local-cxx17.cpp).
struct S { int a, b; };
S s{1, 2};
static auto [a1, b1] = s;
thread_local auto [a2, b2] = s;
static thread_local auto [a3, b3] = s;
int f() {
  static auto [a4, b4] = s;
  thread_local auto [a5, b5] = s;
  return a1 + a2 + a3 + a4 + a5 + b1 + b2 + b3 + b4 + b5;
}
