// [dcl.contract.res]/1-2 (P2900R14): a result-name-introducer declares a
// name for the result object of the function, a const lvalue of its
// return type (a reference for a reference return), scoped to that
// postcondition; another postcondition may reuse the name, and it may
// shadow nothing the function body declares. GCC 16.1 agrees.
//@ expect: compile-ok
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ oracle: gcc
struct P { int a; int get() const { return a; } };
int f() post(r: r > 0) post(r: r < 9) { return 1; }
int& g(int& x) post(r: &r == &x) { return x; }
P h() post(p: p.get() == 1) post(q: q.a == 1) { return {1}; }
const int* k() post(r: r != nullptr) { static int i; return &i; }
template <class T> T t() post(r: r == T{}) { return T{}; }
struct S { int m; int get() const post(r: r == m) { return m; } };
auto lam = []() post(r: r) { return true; };
int use() { int i = 0; return f() + g(i) + h().a + *k() + t<int>() + S{}.get() + lam(); }
