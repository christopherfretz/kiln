// [expr.pre]/4 leaves signed overflow undefined; under -fwrapv
// it wraps, so kiln emits no 'nsw' on signed arithmetic, shifts,
// increments, negation and VLA sizes, and no 'inbounds' on pointer
// arithmetic and array subscripts, as clang (LangOptions::SOB_Defined,
// PointerOverflowDefined). Under clang 22 field GEPs lose 'inbounds'
// too (spec/codegen.md). The Oven is unchanged (spec/driver.md).
//@ args: -fwrapv
int arith(int a, int b, long c) { int r = a + b; r -= a; r *= b; ++r; r--; r = -r; c += a; return r + (int)c + (a << 2); }
int ptrs(int *p, int i, int n) { int *q = p + i; q++; --q; q -= 2; int arr[4] = {}; int vla[n]; vla[i] = 1; return *q + p[i] + arr[i] + vla[i] + (int)(q - p); }
struct S { int a[3]; int b; };
int mem(S *s, int i) { return s->a[i] + s->b + s[1].b; }
unsigned u(unsigned a, unsigned b) { return a + b * 2; }
int f(int n, int i) { int a[n][n]; a[i][i] = 1; return a[0][i]; }
struct E {};
struct F { int a; int b : 3; int c : 5; [[no_unique_address]] E e; int d; char z[4]; };
int fld(F *s, int i) { s->b = 1; E *pe = &s->e; (void)pe; return s->a + s->c + s->d + s->z[i]; }
