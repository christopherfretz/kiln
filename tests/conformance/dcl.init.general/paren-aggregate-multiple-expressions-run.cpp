// [dcl.init.general]/16.6.2.2, /16.5 (P0960): an aggregate or array
// object direct-initialized from a parenthesized expression-list, as
// written and in an instantiated template; a scalar takes one
// expression. The positive twin of dcl.init.ref/paren-multiple-expressions.cpp.
//@ expect: run
//@ std: c++20
//@ stdout: 3 3 5
#include <cstdio>
struct A { int i, j; };
template<class T> int f(T t) { A a(t, t + 1); int b[2](t, t); return a.i + a.j + b[0] + b[1]; }
int main() { A a(1, 2); int x(3); std::printf("%d %d %d\n", a.i + a.j, x, f(1)); }
