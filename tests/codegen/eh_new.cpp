// [expr.new]/28: the deallocation function is called when the
// initialization of a new-expression throws (global, nothrow, class
// specific sized and placement forms; a conditional new-expression).
namespace std { using size_t = decltype(sizeof 0); struct nothrow_t { explicit nothrow_t() = default; }; extern const nothrow_t nothrow; enum class align_val_t : size_t {}; }
void* operator new(std::size_t, const std::nothrow_t&) noexcept;
void operator delete(void*, const std::nothrow_t&) noexcept;
void* operator new(std::size_t, void*) noexcept;
int g(int);
struct G { int x; G(int); ~G(); };
struct H { int x; H(int); };
G* newg() { return new G(g(0)); }
H* newh(bool b) { return b ? new H(g(1)) : nullptr; }
struct P { P(); static void* operator new(std::size_t, int); static void operator delete(void*, int); };
P* newp(int k) { return new (k) P; }
struct Q { Q(); static void* operator new(std::size_t); static void operator delete(void*, std::size_t); };
Q* newq() { return new Q; }
H* newnt() { return new (std::nothrow) H(g(2)); }
int* newint() { return new int(g(3)); }
void* buf;
H* plc() { return new (buf) H(g(4)); }
P* newp2() { return new (1) P; }
