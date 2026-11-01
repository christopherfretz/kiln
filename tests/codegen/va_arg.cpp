// __builtin_va_arg lowering: AAPCS64 (gr/vr offsets, HFA copies, indirect
// records, over-aligned slots) and SysV x86-64 (gp/fp offsets, mixed and
// SSE-pair temporaries, overflow area).
struct S { int a, b; };
struct L { long a, b, c; };
struct D { double x, y; };
struct A16 { alignas(16) long a; long b; };
int fi(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); int r = __builtin_va_arg(ap, int); __builtin_va_end(ap); return r; }
long fl(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); long r = __builtin_va_arg(ap, long); __builtin_va_end(ap); return r; }
double fd(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); double r = __builtin_va_arg(ap, double); __builtin_va_end(ap); return r; }
void* fp(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); void* r = __builtin_va_arg(ap, void*); __builtin_va_end(ap); return r; }
S fs(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); S r = __builtin_va_arg(ap, S); __builtin_va_end(ap); return r; }
L fL(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); L r = __builtin_va_arg(ap, L); __builtin_va_end(ap); return r; }
D fD(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); D r = __builtin_va_arg(ap, D); __builtin_va_end(ap); return r; }
A16 fA(int n, ...) { __builtin_va_list ap; __builtin_va_start(ap, n); A16 r = __builtin_va_arg(ap, A16); __builtin_va_end(ap); return r; }
struct F3 { float a, b, c; };
struct M { int a; double d; };
struct FF { float a, b; };
struct E {};
int pi(__builtin_va_list ap) { return __builtin_va_arg(ap, int); }
float pf(__builtin_va_list ap) { return __builtin_va_arg(ap, float); }
long double pld(__builtin_va_list ap) { return __builtin_va_arg(ap, long double); }
__int128 p128(__builtin_va_list ap) { return __builtin_va_arg(ap, __int128); }
F3 pF3(__builtin_va_list ap) { return __builtin_va_arg(ap, F3); }
M pM(__builtin_va_list ap) { return __builtin_va_arg(ap, M); }
FF pFF(__builtin_va_list ap) { return __builtin_va_arg(ap, FF); }
E pE(__builtin_va_list ap) { return __builtin_va_arg(ap, E); }
bool pb(__builtin_va_list ap) { return __builtin_va_arg(ap, bool); }
void (E::*pmf(__builtin_va_list ap))() { return __builtin_va_arg(ap, void (E::*)()); }
struct V2 { double a; double b; };
long two(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  long a = __builtin_va_arg(ap, long);
  __builtin_va_arg(ap, int);
  V2 v = __builtin_va_arg(ap, V2);
  a += (long)__builtin_va_arg(ap, V2).b;
  __builtin_va_end(ap);
  return a + (long)v.a;
}
