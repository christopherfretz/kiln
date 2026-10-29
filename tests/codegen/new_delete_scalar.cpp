// new/delete of scalars ([expr.new], [expr.delete]): _Znwm/_ZdlPvm,
// value- and list-initialization, reserved placement new, alignas(32) types
// through the std::align_val_t operators.
int *a() { return new int; }
int *b() { return new int(3); }
int *c(int n) { return new int[n]; }
int *d(int n) { return new int[n](); }
int *e() { return new int[3]{1, 2}; }
int *f(int n) { return new int[n]{1, 2}; }
void g(int *p) { delete p; }
void h(int *p) { delete[] p; }
double *i() { return new double{}; }
int (*j(int n))[4] { return new int[n][4]; }
inline void *operator new(unsigned long, void *p) noexcept { return p; }
void *k(void *p) { return new (p) int(5); }
struct alignas(32) A { int x[8]; };
A *l() { return new A{}; }
void m(A *p) { delete p; }
