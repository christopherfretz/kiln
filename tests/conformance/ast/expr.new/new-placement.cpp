//@ expect: ast
inline void *operator new(unsigned long, void *p) noexcept { return p; }
void *operator new[](unsigned long, int, int);
void *operator new(unsigned long, short, char) noexcept;
int *a(int n) { return new ((short)n, 'x') int(1); }
int *b(void *buf) { return new (buf) int; }
int *c(int n) { return new (1, 2) int[n]; }
struct P { int x, y; };
P *d(void *buf) { return new (buf) P{7, 8}; }
double *e() { return new auto(2.5); }
