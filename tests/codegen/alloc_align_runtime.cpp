// [basic.stc.dynamic.allocation]/2, [new.delete.single]: an aligned
// allocation function's alignment argument that is not a constant power
// of two becomes an `llvm.assume` with an "align" bundle after the call
// (clang `AllocAlignAttrEmitter`); the GNU `alloc_align` attribute too.
namespace std { using size_t = decltype(sizeof 0); enum class align_val_t : size_t {}; struct nothrow_t { explicit nothrow_t() = default; }; extern const nothrow_t nothrow; }
void* operator new(std::size_t, std::align_val_t, const std::nothrow_t&) noexcept;
void* a(std::size_t n, std::size_t al) { return ::operator new(n, static_cast<std::align_val_t>(al)); }
void* b(std::size_t n, std::align_val_t al) { return ::operator new[](n, al); }
void* c(std::size_t n, std::align_val_t al) { return ::operator new(n, al, std::nothrow); }
void* d(std::size_t n) { return ::operator new(n, std::align_val_t(3)); }
void* e(std::size_t n) { return ::operator new(n, std::align_val_t(64)); }
void* f(std::size_t n, std::align_val_t al) { return __builtin_operator_new(n, al); }
char* g(std::size_t n, std::align_val_t al) { return new (al) char[n]; }
struct D { ~D(); };
void* h(std::size_t n, std::align_val_t al) { D x; return ::operator new(n, al); }
__attribute__((alloc_align(2))) void* m(int, int);
__attribute__((alloc_align(2))) void* m2(int, unsigned short);
void* i(int x) { return m(1, x); }
void* j(unsigned short x) { return m2(1, x); }
void* k() { return m(1, 6); }
struct S { __attribute__((alloc_align(2))) void* q(int); };
void* l(S& s, int x) { return s.q(x); }
// Virtual calls carry the allocation attributes of the method the call
// names: `alloc_align` (an assume, or `align N` for a constant),
// `alloc_size` and `malloc`.
struct V {
  virtual void* al(unsigned long a) __attribute__((alloc_align(2)));
  virtual void* al2(int x, unsigned long a) __attribute__((alloc_align(3)));
  virtual void* sz(unsigned long n) __attribute__((alloc_size(2)));
  virtual void* ml(unsigned long n) __attribute__((malloc));
};
void* v1(V* a, unsigned long n) { return a->al(n); }
void* v2(V* a) { return a->al(16); }
void* v3(V* a, unsigned long n) { return a->al2(1, n); }
void* v4(V* a, unsigned long n) { return a->sz(n); }
void* v5(V* a, unsigned long n) { return a->ml(n); }
void* v6(V& a, unsigned long n) { return a.al(n); }
void* v7(V* a, unsigned long n) { return a->V::al(n); }
