//@ args: -std=gnu++26
// clang EmitBuiltinExpr (WP-1073): in the GNU dialects the library
// functions alloca, bzero, bcopy, mempcpy, finite / finitef / finitel and
// __finite / __finitef / __finitel are builtins that clang lowers inline:
// a dynamic `alloca` aligned to __BIGGEST_ALIGNMENT__, memset with 0,
// memmove with the operands swapped, memcpy plus the length, and the
// isfinite compare. `memalign` gets `allocsize(1)` and `alloc_align(0)`.
// Taking a builtin's address names the library function.
typedef __SIZE_TYPE__ size_t;
extern "C" {
void* alloca(size_t);
void bzero(void*, size_t);
void bcopy(const void*, void*, size_t);
void* mempcpy(void*, const void*, size_t);
int finite(double);
int finitef(float);
int finitel(long double);
int __finite(double);
int __finitef(float);
int __finitel(long double);
void* memalign(size_t, size_t);
}
void use(void*);
void f1(size_t n) {
  for (size_t i = 0; i < 3; ++i) use(alloca(n + i));
}
void f2(void* p, size_t n) { bzero(p, n); }
void f3(const void* s, void* d, size_t n) { bcopy(s, d, n); }
void* f4(void* d, const void* s, size_t n) { return mempcpy(d, s, n); }
int f5(double d, float f, long double l) {
  return finite(d) + finitef(f) + finitel(l) + __finite(d) + __finitef(f) + __finitel(l);
}
void* f6(size_t a, size_t n) { return memalign(a, n); }
void* (*p1)(size_t) = alloca;
void (*p2)(void*, size_t) = bzero;
int (*p3)(double) = finite;
