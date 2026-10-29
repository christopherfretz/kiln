//@ expect: ast
int *a() { return new int; }
int *b() { return new int(3); }
int *c() { return new int{4}; }
int *d() { return new int(); }
long *e() { return ::new long{}; }
const int *f() { return new const int(5); }
int **g(int *p) { return new int *(p); }
