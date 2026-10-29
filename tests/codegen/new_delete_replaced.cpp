// User-replaced global operators and delete forms ([expr.delete]):
// delete of pointer-to-array, delete[], sized delete.
extern "C" void *my_alloc(unsigned long);
extern "C" void my_free(void *) noexcept;
void *operator new(unsigned long n) { return my_alloc(n); }
void operator delete(void *p) noexcept { my_free(p); }
void operator delete(void *p, unsigned long) noexcept { my_free(p); }
int *a() { return new int[3]{1, 2, 3}; }
int (*b(int n))[4] { return new int[n][4]{{1}, {2, 3}}; }
char buf[64];
inline void *operator new(unsigned long, void *p) noexcept { return p; }
int *c() { return new (buf) int(7); }
long *d(long *q) { return new (q) long(8); }
struct P { char c; int x[3]; };
P *e(int n) { return new P[n]{{1, {2}}}; }
void g(int (*p)[4]) { delete[] p; }
void h(int (*p)[4]) { delete p; }
