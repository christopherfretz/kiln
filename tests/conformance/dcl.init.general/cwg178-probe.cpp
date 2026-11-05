// [dcl.init.general]/9 (C++03, CWG178): value-initialization; `T()`
// zero-initializes a class without a user-provided constructor before
// running its implicit constructor. WP-646 feature probe.
//@ expect: run
//@ std: c++03
//@ strict-diags
//@ stdout: 0 0 0 5 0
extern "C" int printf(const char*, ...);
struct M { int k; M() : k(5) {} };
struct P { int i; double d; };
struct Q { int i; M m; };
int main() {
  P p = P();
  Q q = Q();
  int n = int();
  printf("%d %d %d %d %d\n", p.i, (int)p.d, q.i, q.m.k, n);
}
