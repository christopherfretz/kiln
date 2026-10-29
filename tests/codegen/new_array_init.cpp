// Array new ([expr.new]/7-9): umul.with.overflow sizing, the -1 size on
// overflow or too few elements, memset/loop initialization of the tail.
int *e() { return new int[3]{1, 2}; }
int *f(int n) { return new int[n]{1, 2}; }
int (*j(int n))[4] { return new int[n][4]{{1},{2,3}}; }
int*z(){return new int[]{1,2,3};}
struct S { int a; double b; };
int (*g(int n))[2] { return new int[n][2]{{1, 2}, {3}}; }
S *h(int n) { return new S[n]{{1, 2.0}}; }
S *j2() { return new S{4}; }
int *z0() { return new int[0]; }
int *z5() { return new int[5](); }
int (*m(int n))[3] { return new int[n][3](); }
struct V { int a; int b; };
V *vv(int n) { return new V[n]{{1, 2}, {}}; }
