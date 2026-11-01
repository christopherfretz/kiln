// Allocation attributes (WP-170): the replaceable global allocation
// functions declared as libstdc++'s <new> declares them (GNU `malloc` ->
// `noalias` on the declaration, `alloc_size` -> `allocsize`), the
// implicit attributes of direct calls (clang `ConstructAttributeList`:
// `noalias`, `nonnull` unless nothrow, `allocsize(0)`, `align N` from a
// constant std::align_val_t), the C library allocation functions clang
// knows as builtins (implicit `allocsize`, `aligned_alloc`'s implicit
// `alloc_align`), and the nothrow null check of a trivial class.
using size_t = decltype(sizeof 0);
namespace std {
enum class align_val_t : size_t {};
struct nothrow_t { explicit nothrow_t() = default; };
extern const nothrow_t nothrow;
}
void *operator new(size_t) __attribute__((__alloc_size__(1), __malloc__));
void *operator new[](size_t) __attribute__((__alloc_size__(1), __malloc__));
void operator delete(void *) noexcept;
void operator delete[](void *) noexcept;
void *operator new(size_t, const std::nothrow_t &) noexcept
    __attribute__((__alloc_size__(1), __malloc__));
void *operator new(size_t, std::align_val_t) __attribute__((__alloc_size__(1), __malloc__));
void *operator new[](size_t, std::align_val_t, const std::nothrow_t &) noexcept
    __attribute__((__alloc_size__(1), __malloc__));
void operator delete(void *, std::align_val_t) noexcept;
extern "C" {
void *malloc(size_t) noexcept __attribute__((__malloc__));
void *calloc(size_t, size_t) noexcept __attribute__((__malloc__));
void *realloc(void *, size_t) noexcept;
void *aligned_alloc(size_t, size_t) noexcept __attribute__((__malloc__));
}
struct P { int a; };
struct Q { int a; Q(); };
struct alignas(64) A { int a; };
int *n1() { return new int; }
int *n2(size_t n) { return new int[n]; }
void n3(int *p) { delete p; }
void n4(int *p) { delete[] p; }
A *n5() { return new A; }
void n6(A *p) { delete p; }
int *t1() { return new (std::nothrow) int; }
P *t2() { return new (std::nothrow) P; }
Q *t3() { return new (std::nothrow) Q; }
P *t4() { return new (std::nothrow) P(); }
void *d1(size_t n) { return ::operator new(n); }
void *d2(size_t n) { return ::operator new(n, std::align_val_t{32}); }
void *d3(size_t n) { return ::operator new[](n, std::align_val_t{64}, std::nothrow); }
void d4(void *p) { ::operator delete(p, std::align_val_t{32}); }
void *c1(size_t n) { return malloc(n); }
void *c2(size_t n) { return calloc(n, 2); }
void *c3(void *p, size_t n) { return realloc(p, n); }
void *c4(size_t n) { return aligned_alloc(16, n); }
void *mine(size_t) __attribute__((malloc));
void *mine(size_t n) { return malloc(n); }
void *g1() { return mine(4); }
void *pool(unsigned, size_t, size_t) __attribute__((alloc_size(2, 3), alloc_align(1)));
void *g2() { return pool(8, 4, 100); }
struct Arena {
  void *get(int, size_t) __attribute__((alloc_size(3), alloc_align(2)));
};
void *g3(Arena &a) { return a.get(16, 8); }
