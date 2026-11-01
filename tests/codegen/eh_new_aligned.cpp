// [expr.new]/28: when the initialization of an aligned new-expression
// throws, the deallocation function is the one matching the aligned
// allocation function as a placement form: the unsized aligned
// `operator delete` (clang treats `(size_t, align_val_t)` as placement).
namespace std { using size_t = decltype(sizeof 0); enum class align_val_t : size_t {}; struct nothrow_t { explicit nothrow_t() = default; }; extern const nothrow_t nothrow; }
struct alignas(32) A { A(); int v; };
A* a() { return new A; }
A* b(int n) { return new A[n]; }
struct alignas(32) B { B(); ~B(); int v; };
B* c() { return new B; }
B* d(int n) { return new B[n]; }
void* operator new(std::size_t, std::align_val_t, const std::nothrow_t&) noexcept;
void operator delete(void*, std::align_val_t, const std::nothrow_t&) noexcept;
A* e() { return new (std::nothrow) A; }
struct alignas(32) C { C(); void* operator new(std::size_t, std::align_val_t); void operator delete(void*, std::align_val_t); void operator delete(void*, std::size_t, std::align_val_t); };
C* f() { return new C; }
