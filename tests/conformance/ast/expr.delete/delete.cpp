//@ expect: ast
struct S { int a; };
void a(int *p) { delete p; }
void b(int *p) { delete[] p; }
void c(S *s) { ::delete s; }
void d(int (*m)[3]) { delete[] m; }
void e(const double *q) { ::delete[] q; }
