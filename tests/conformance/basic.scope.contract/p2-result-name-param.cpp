// [basic.scope.contract]/2.1 (P2900R14): a result name that is not
// name-independent and potentially conflicts with a parameter of its
// function makes the program ill-formed; `_` is name-independent. Found
// running GCC's C++ testsuite (g++.dg/contracts).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
//@ diag: 13:28 error "redefinition of parameter 'r'"
//@ diag: 14:42 error "redefinition of parameter 'x'"
//@ diag: 16:30 error "redefinition of parameter 'r'"
//@ diag: 17:32 error "redefinition of parameter 'r'"
int bad(int a, int r) post(r: r > 5) { return a; }
template <class T> int bad_t(int x) post(x: x > 0) { return x; }
int ok(int a) post(_: true) post(r: r > 0) { return a; }
struct S { int m(int r) post(r: r > 0); };
auto l = [](int r) -> int post(r: r > 0) { return r; };
int use() { return bad_t<int>(1) + ok(1) + l(1); }
