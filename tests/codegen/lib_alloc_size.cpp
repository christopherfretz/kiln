// clang Sema::AddKnownFunctionAttributes (WP-1073): the library allocation
// functions get an implicit `alloc_size` (malloc: 1; calloc: 1, 2; realloc:
// 2; aligned_alloc: 2, with `alloc_align(1)`), as `allocsize` on the
// declaration and the call. Only a declaration with the library signature
// at global scope (or an `extern "C"` one in a namespace) is the builtin:
// a mismatched one is an ordinary function. `memalign` is GNU-only.
typedef __SIZE_TYPE__ size_t;
extern "C" {
void* malloc(size_t);
void* calloc(size_t, size_t);
void* realloc(void*, size_t);
void* aligned_alloc(size_t, size_t);
void* memalign(size_t, size_t);
}
void* f1(size_t n) { return malloc(n); }
void* f2(size_t n) { return calloc(n, 4); }
void* f3(void* p, size_t n) { return realloc(p, n); }
void* f4(size_t a, size_t n) { return aligned_alloc(a, n); }
void* f5(size_t n) { return aligned_alloc(64, n); }
void* f6(size_t a, size_t n) { return memalign(a, n); }
namespace N { extern "C" void* malloc(size_t); }
void* f7(size_t n) { return N::malloc(n); }
