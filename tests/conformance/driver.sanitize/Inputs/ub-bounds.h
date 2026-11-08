// Subscripts for the array-bounds check (clang EmitBoundsCheck; WP-871).
namespace n1 {
typedef int v4 __attribute__((vector_size(16)));
struct S { int a[4]; int n; int fam[1]; };
union U { int x[1]; int y[2]; };
int g[10];
int f1(int i) { return g[i]; }
int *f2(unsigned i) { return &g[i]; }
void f3(long i, int v) { g[i] = v; }
int f4(int i, int j) { int m[3][5] = {}; return m[i][j]; }
int f5(S *s, int i) { return s->a[i] + s->fam[i]; }
int f6(U *u, int i) { return u->x[i] + u->y[i]; }
int *f7(int i) { return g + i; }
int *f8(int i) { return g - i; }
int *f9(int *p, int i) { return p + i; }
int f10(v4 v, int i) { return v[i]; }
void f11(v4 &v, int i) { v[i] = 1; }
int f12(char c) { return g[c]; }
int f13(int n, int i) { int a[n]; a[0] = 0; return a[i]; }
int f14(int i) { return i[g]; }
int f15(int n, int i, int j) { int a[n][n]; a[1][1] = 0; return a[i][j]; }
int f16(int i) { int z[0]; return z[i]; }
int f17(int i) { return (g)[i]; }
int f18(unsigned long long i) { return g[i]; }
} // namespace n1
namespace n2 {
int g[10];
struct T { int a[3]; int b; };
int f1(int (&r)[6], int i) { return r[i]; }
char f2(int i) { return "abc"[i]; }
void f3(int *&p, int i) { p += i; }
int f4(T &t, int i) { return t.a[i]; }
template <class X> int f5(X &x, int i) { return x.a[i]; }
int f6(T t, int i) { return f5(t, i); }
int f7(int i) { return *(g + i); }
int f8(int i) { int *p = &g[i]; return p[1]; }
int f9(int i) { g[i]++; return g[i] += 2; }
int f10(bool b, short s) { return g[b] + g[s]; }
int *f11(int i) { return 2 + g; }
int f12(int i) { static thread_local int tl[5]; return tl[i]; }
} // namespace n2
