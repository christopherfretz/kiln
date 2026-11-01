// [expr.new]/16, [expr.delete]/6-11: array new of a class with a
// non-trivial destructor (or a class-specific sized `operator delete[]`)
// stores the element count in an Itanium array cookie, padded to the
// element alignment; `delete[]` reads it back, destroys the elements in
// reverse order and calls the sized deallocation function even if a
// destructor throws. No cookie for the reserved placement form.
namespace std { using size_t = decltype(sizeof 0); struct nothrow_t { explicit nothrow_t() = default; }; extern const nothrow_t nothrow; }
void* operator new[](std::size_t, const std::nothrow_t&) noexcept;
void* operator new[](std::size_t, void*) noexcept;
struct D { D(); ~D(); int v; };
D* a(int n) { return new D[n]; }
D* a3() { return new D[3]; }
void da(D* p) { delete[] p; }
D (*m(int n))[3] { return new D[n][3]; }
void dm(D (*p)[3]) { delete[] p; }
D* nt(int n) { return new (std::nothrow) D[n]; }
struct T { ~T(); char v; };
T* zt(int n) { return new T[n](); }
void dt(T* p) { delete[] p; }
struct T16 { ~T16(); long double v; };
T16* z16(int n) { return new T16[n](); }
void d16(T16* p) { delete[] p; }
struct alignas(64) A64 { ~A64(); int v; };
A64* z64(int n) { return new A64[n]; }
void d64(A64* p) { delete[] p; }
struct E { char v; void operator delete[](void*, std::size_t); };
E* e(int n) { return new E[n]; }
void de(E* p) { delete[] p; }
struct C { C(); ~C(); int v; void* operator new[](std::size_t); void operator delete[](void*, std::size_t); };
C* c(int n) { return new C[n]; }
void dc(C* p) { delete[] p; }
struct X { ~X() noexcept(false); int v; };
void dx(X* p) { delete[] p; }
D* pl(void* b) { return new (b) D[2]; }
