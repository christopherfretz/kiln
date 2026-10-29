//@ expect: ast
int *a() { return new int[4]; }
int *b(int n) { return new int[n]{1, 2}; }
short *c(unsigned n) { return new short[n](); }
int (*d(long n))[3] { return new int[n][3]; }
char *e() { return new char[2]{}; }
struct P { int x, y; };
P *f() { return new P[2]{{1, 2}}; }
