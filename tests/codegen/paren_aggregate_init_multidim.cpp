// [dcl.init.general]/16.5, /16.6.2.2, /9 (P0960; WP-182): parenthesized
// initialization of multidimensional arrays: braced-init-list rows, rows
// of classes value-initialized by one array construction (with unwind
// cleanup of the constructed elements), a value-initialized member that is
// an array of classes, a mem-initializer `s()` of a 2-D class array,
// array new with a runtime outer bound, and constant globals.
struct S { S() : v(7) {} S(int x) : v(x) {} ~S(); int v; };
struct T { int v; };
struct U { U() : v(7) {} U(int x) : v(x) {} int v; };
struct D { int x; S s[2]; };
struct E { E() : s() {} S s[2][2]; };
int f() { S a[2][3]({S(1)}); return a[1][2].v; }
int g() { T a[2][3]({T{1}}); return a[1][2].v; }
int k() { S a[3][2]({S(1), S(2)}, {S(3)}); return a[2][1].v + a[1][1].v; }
int f2() { D d(1); return d.s[1].v; }
int g2() { E e; return e.s[1][1].v; }
U (*nw(int n))[2] { return new U[n][2]({U(4)}); }
int (*nwi())[2] { return new int[3][2]({1, 2}, {3}); }
U gs[2][2]({U(5)});
int gi[2][3]({1, 2}, {3});
struct A { int x, y; };
A ga[2]({1, 2});
struct B { int b[2]; A a; };
B gb({1, 2}, {3});
