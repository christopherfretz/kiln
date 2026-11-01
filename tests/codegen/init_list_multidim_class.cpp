// [dcl.init.aggr] (WP-182): brace initialization of multidimensional class
// arrays. clang's CheckAggExprForMemSetUse never zeroes an object whose
// base element class has a user-declared constructor first (the
// value-initialized elements run the constructor); a class without one
// still takes the memset when 3/4 of the bytes are zero.
struct S { S() : v(7) {} S(int x) : v(x) {} ~S(); int v; };
struct T { int v; };
int f() { S a[2][3] = {{S(1)}}; return a[1][2].v; }
int g() { S a[2][3] = {}; return a[1][2].v; }
int h() { S a[2][3] = {{1, 2}, {3}}; return a[1][2].v; }
int i() { T a[2][3] = {{1}}; return a[1][2].v; }
int j() { S a[][2] = {{1}, {2}}; return a[1][1].v; }
struct M { S m[2][2]; };
int k() { M m{}; M n = {{{1}}}; return m.m[1][1].v + n.m[0][1].v; }
struct Z { int a, b, c; };
int z() { Z a[2][2] = {{{1}}}; return a[1][1].c; }
