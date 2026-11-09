// clang Builtin::Context (WP-1073): alloca, bzero, bcopy, mempcpy, finite
// and __finite are library builtins only in the GNU dialects; under
// -std=c++26 they are ordinary calls.
typedef __SIZE_TYPE__ size_t;
extern "C" {
void* alloca(size_t);
void bzero(void*, size_t);
void bcopy(const void*, void*, size_t);
void* mempcpy(void*, const void*, size_t);
int finite(double);
int __finite(double);
}
void* f1(size_t n) { return alloca(n); }
void f2(void* p, size_t n) { bzero(p, n); }
void f3(const void* s, void* d, size_t n) { bcopy(s, d, n); }
void* f4(void* d, const void* s, size_t n) { return mempcpy(d, s, n); }
int f5(double d) { return finite(d) + __finite(d); }
