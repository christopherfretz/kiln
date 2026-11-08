// Pointer arithmetic for the pointer-overflow check (clang
// EmitCheckedInBoundsGEP; WP-871).
int a[4];
struct S { int x; int y[3]; };
int f0(int i) { return a[i]; }
int f1() { return a[2]; }
int f2(int *p, int i) { return p[i]; }
int f3(int *p, unsigned i) { return p[i]; }
int *f4(int *p, long n) { return p + n; }
int *f5(int *p, unsigned long n) { return p - n; }
int *f6(int *p) { return ++p; }
int *f7(int *p) { return p--; }
int f8(S *s, int i) { return s->y[i]; }
int f9() { int b[4] = {}; return b[1]; }
int f10(int *p) { return p[0]; }
int f11(int *p) { return *(p + 3); }
void f12(int *&p, int n) { p += n; p -= 2; }
char f13(int n, int i) { char b[n]; b[0] = 0; return b[i]; }
void *f15(void *p, int n) { return (char*)p + n; }
